use crate::{
    cache::Cache,
    inventory::{self, BLADES, SCROLLS},
    market::validate_time,
    types::*,
    upstream::Upstream,
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
#[derive(Default)]
struct Index {
    listings: Vec<Value>,
    updated: i64,
    retrieved: i64,
    total_pages: usize,
    scanned_pages: usize,
    total_auctions: usize,
    scanned_auctions: usize,
    complete: bool,
    invalid: usize,
    retries: usize,
    recovered: usize,
    failed: usize,
    warnings: Vec<String>,
    seen: HashSet<String>,
}
fn validate(t: &Value) -> Result<i64> {
    validate_time(
        t,
        now(),
        "AUCTION_SNAPSHOT_EXPIRED",
        "The upstream auction snapshot is not recent enough.",
    )
}
pub fn decode_listing(s: &str) -> Option<Value> {
    let decoded = inventory::decode_nbt(s, 128 * 1024, 1024 * 1024).ok()?;
    let items = decoded["i"].as_array().filter(|a| a.len() == 1)?;
    let item = &items[0];
    let extra = at(item, "tag.ExtraAttributes");
    let id = extra["id"]
        .as_str()
        .filter(|s| re("^[A-Z0-9_:.-]{1,96}$", s))?;
    let count = n(&item["Count"]).filter(|n| n.fract() == 0. && (1. ..=64.).contains(n))?;
    let reforge = extra["modifier"]
        .as_str()
        .filter(|s| re("^[a-z0-9_]{1,64}$", s));
    let stars = n(&extra["upgrade_level"])
        .or(n(&extra["dungeon_item_level"]))
        .unwrap_or(0.);
    if stars.fract() != 0. || !(0. ..=15.).contains(&stars) {
        return None;
    }
    let pet_type = if id == "PET" {
        let s = extra["petInfo"].as_str().filter(|s| s.len() <= 4096)?;
        let p: Value = serde_json::from_str(s).ok()?;
        Some(
            p["type"]
                .as_str()
                .filter(|s| re("^[A-Z0-9_]{1,64}$", s))?
                .to_owned(),
        )
    } else {
        None
    };
    Some(
        json!({"id":id,"count":count,"reforge":reforge,"stars":stars,"petType":pet_type,"variants":inventory::variants(id,extra)}),
    )
}
async fn fetch_page(
    u: &Upstream,
    page: usize,
    index: &mut Index,
    deadline: Instant,
) -> Result<Value> {
    let result = tokio::time::timeout_at(deadline.into(), u.auction_page(page))
        .await
        .unwrap_or_else(|_| {
            Err(AppError::retry(
                "UPSTREAM_TIMEOUT",
                "The upstream request exceeded its time limit.",
                15,
            ))
        });
    match result {
        Ok(v) => Ok(v),
        Err(e) => {
            if !e.transient
                || !["UPSTREAM_TIMEOUT", "UPSTREAM_UNAVAILABLE"].contains(&e.code.as_str())
                || index.retries >= 2
                || deadline.saturating_duration_since(Instant::now()) < Duration::from_millis(10500)
            {
                index.failed += 1;
                return Err(e);
            }
            index.retries += 1;
            tokio::time::sleep(Duration::from_millis(250)).await;
            let result = tokio::time::timeout_at(deadline.into(), u.auction_page(page))
                .await
                .unwrap_or_else(|_| {
                    Err(AppError::retry(
                        "UPSTREAM_TIMEOUT",
                        "The upstream request exceeded its time limit.",
                        15,
                    ))
                });
            if result.is_ok() {
                index.recovered += 1
            } else {
                index.failed += 1
            }
            result
        }
    }
}
async fn consume(
    raw: &Value,
    expected: usize,
    index: &mut Index,
    deadline: Instant,
    diagnostics: std::sync::Arc<crate::diagnostics::Diagnostics>,
) -> Result<bool> {
    let updated = validate(&raw["lastUpdated"])?;
    let total = raw["totalPages"]
        .unsigned()
        .filter(|n| (1..=128).contains(n));
    let auctions = raw["totalAuctions"].unsigned().filter(|n| *n <= 128000);
    let Some(a) = raw["auctions"].as_array().filter(|a| a.len() <= 1000) else {
        return Err(AppError::retry(
            "UPSTREAM_INVALID",
            "Auction page metadata is invalid.",
            15,
        ));
    };
    if raw["page"].unsigned() != Some(expected as u64) || total.is_none() || auctions.is_none() {
        return Err(AppError::retry(
            "UPSTREAM_INVALID",
            "Auction page metadata is invalid.",
            15,
        ));
    }
    if expected == 0 {
        index.updated = updated;
        index.total_pages = total.unwrap() as usize;
        index.total_auctions = auctions.unwrap() as usize;
    } else if updated != index.updated
        || total.unwrap() as usize != index.total_pages
        || auctions.unwrap() as usize != index.total_auctions
    {
        index.complete = false;
        index.warnings.push(
            "Auction pages changed during the scan; pages from another snapshot were omitted."
                .into(),
        );
        return Ok(false);
    }
    index.scanned_pages += 1;
    index.scanned_auctions += a.len();
    let bins = a
        .iter()
        .filter(|a| {
            let Some(id) = a["uuid"].as_str().filter(|s| re("(?i)^[a-f0-9]{32}$", s)) else {
                index.complete = false;
                index.invalid += 1;
                return false;
            };
            if !index.seen.insert(id.to_owned()) {
                index.complete = false;
                index.invalid += 1;
                return false;
            }
            if !a["bin"].is_boolean() || !a["claimed"].is_boolean() || n(&a["end"]).is_none() {
                index.complete = false;
                index.invalid += 1;
                return false;
            }
            a["bin"] == true
                && a["claimed"] == false
                && n(&a["end"]).is_some_and(|n| n > now() as f64)
        })
        .cloned()
        .collect::<Vec<_>>();
    let encoded = bins
        .iter()
        .map(|a| {
            a["item_bytes"]
                .as_str()
                .or(at(a, "item_bytes.data").as_str())
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let parse_deadline = deadline.min(Instant::now() + Duration::from_secs(10));
    let queued = Instant::now();
    let parsed = tokio::task::spawn_blocking(move || {
        diagnostics.measure("worker_queue_wait", queued.elapsed().as_secs_f64() * 1000.);
        let _span = diagnostics.span("auction_decode");
        crate::diagnostics::scoped(diagnostics.clone(), || {
            let mut bytes = 0;
            encoded
                .iter()
                .map(|s| {
                    let s = s.as_ref()?;
                    bytes += s.len();
                    if bytes > 16 * 1024 * 1024 || Instant::now() >= parse_deadline {
                        return None;
                    }
                    decode_listing(s)
                })
                .collect::<Vec<_>>()
        })
    })
    .await
    .map_err(|_| {
        AppError::retry(
            "AUCTION_DECODE_FAILED",
            "Auction decoder could not finish safely.",
            15,
        )
    })?;
    if Instant::now() >= deadline {
        return Err(AppError::retry(
            "AUCTION_SCAN_TIMEOUT",
            "Auction scan reached its time budget.",
            15,
        ));
    }
    for (a, item) in bins.iter().zip(parsed) {
        let Some(item) = item else {
            index.complete = false;
            index.invalid += 1;
            continue;
        };
        let price = n(&a["starting_bid"]);
        let rarity = a["tier"].as_str().filter(|s| re("^[A-Z_ ]{1,16}$", s));
        if price.is_none_or(|n| n <= 0.) || rarity.is_none() {
            index.complete = false;
            index.invalid += 1;
            continue;
        }
        index.listings.push(json!([
            item["id"],
            price,
            item["count"],
            rarity.unwrap().replace(' ', "_"),
            item["reforge"],
            item["stars"],
            a["end"],
            item["petType"],
            item["variants"]
        ]));
    }
    Ok(true)
}
pub async fn scan(u: Arc<Upstream>, budget: Duration) -> Result<Value> {
    let deadline = Instant::now() + budget;
    let mut index = Index {
        retrieved: now(),
        complete: true,
        ..Default::default()
    };
    let result = async {
        let raw = fetch_page(&u, 0, &mut index, deadline).await?;
        consume(&raw, 0, &mut index, deadline, u.diagnostics.clone()).await?;
        for page in (1..index.total_pages).step_by(2) {
            if Instant::now() >= deadline {
                return Err(AppError::retry(
                    "AUCTION_SCAN_TIMEOUT",
                    "Auction scan reached its time budget.",
                    15,
                ));
            }
            let mut pages = vec![page];
            if page + 1 < index.total_pages {
                pages.push(page + 1)
            }
            // Read at most two public pages concurrently. Retry decisions stay serialized
            // to share the scan-wide retry budget; each failed page gets one retry at most.
            let first = tokio::time::timeout_at(deadline.into(), u.auction_page(page));
            let second = async {
                if pages.len() == 2 {
                    Some(tokio::time::timeout_at(deadline.into(), u.auction_page(page + 1)).await)
                } else {
                    None
                }
            };
            let (a, b) = tokio::join!(first, second);
            let mut results = vec![a.unwrap_or_else(|_| {
                Err(AppError::retry(
                    "UPSTREAM_TIMEOUT",
                    "The upstream request exceeded its time limit.",
                    15,
                ))
            })];
            if let Some(b) = b {
                results.push(b.unwrap_or_else(|_| {
                    Err(AppError::retry(
                        "UPSTREAM_TIMEOUT",
                        "The upstream request exceeded its time limit.",
                        15,
                    ))
                }))
            }
            let mut stop = false;
            for (p, result) in pages.iter().zip(results) {
                let result = match result {
                    Ok(v) => Ok(v),
                    Err(e) => {
                        if e.transient
                            && ["UPSTREAM_TIMEOUT", "UPSTREAM_UNAVAILABLE"]
                                .contains(&e.code.as_str())
                            && index.retries < 2
                            && deadline.saturating_duration_since(Instant::now())
                                >= Duration::from_millis(10500)
                        {
                            index.retries += 1;
                            tokio::time::sleep(Duration::from_millis(250)).await;
                            let retry =
                                tokio::time::timeout_at(deadline.into(), u.auction_page(*p))
                                    .await
                                    .unwrap_or_else(|_| {
                                        Err(AppError::retry(
                                            "UPSTREAM_TIMEOUT",
                                            "The upstream request exceeded its time limit.",
                                            15,
                                        ))
                                    });
                            if retry.is_ok() {
                                index.recovered += 1
                            } else {
                                index.failed += 1
                            }
                            retry
                        } else {
                            index.failed += 1;
                            Err(e)
                        }
                    }
                };
                match result {
                    Ok(raw) => {
                        if !consume(&raw, *p, &mut index, deadline, u.diagnostics.clone()).await? {
                            stop = true
                        }
                    }
                    Err(_) => {
                        index.complete = false;
                        index
                            .warnings
                            .push("One or more auction pages could not be retrieved.".into());
                        stop = true
                    }
                }
            }
            if stop {
                break;
            }
        }
        Ok(())
    }
    .await;
    if let Err(e) = result {
        if index.scanned_pages == 0 {
            return Err(e);
        }
        index.complete = false;
        index
            .warnings
            .push("The scan stopped before complete coverage could be established.".into());
    }
    if index.scanned_pages != index.total_pages || index.scanned_auctions != index.total_auctions {
        index.complete = false
    }
    index.retrieved = now();
    validate(&json!(index.updated))?;
    let mut seen = HashSet::new();
    index.warnings.retain(|w| seen.insert(w.clone()));
    Ok(
        json!({"listings":index.listings,"updatedAt":index.updated,"retrievedAt":index.retrieved,"totalPages":index.total_pages,"scannedPages":index.scanned_pages,"totalAuctions":index.total_auctions,"scannedAuctions":index.scanned_auctions,"complete":index.complete,"invalidListings":index.invalid,"retryAttempts":index.retries,"recoveredPageFetches":index.recovered,"failedPageFetches":index.failed,"warnings":index.warnings}),
    )
}
pub fn validate_filters(item: &str, filters: &Value) -> Result<String> {
    let bad = || {
        AppError::new(
            "INVALID_AUCTION_QUERY",
            "Use an exact item ID and supported rarity, reforge, stars, pet type, pet level or scroll filters.",
        )
    };
    if !re("^[a-zA-Z0-9_:.-]{1,96}$", item)
        || has(filters, "rarity")
            && !filters["rarity"].as_str().is_some_and(|r| {
                [
                    "COMMON",
                    "UNCOMMON",
                    "RARE",
                    "EPIC",
                    "LEGENDARY",
                    "MYTHIC",
                    "DIVINE",
                    "SPECIAL",
                    "VERY_SPECIAL",
                ]
                .contains(&r)
            })
        || has(filters, "reforge")
            && !filters["reforge"]
                .as_str()
                .is_some_and(|s| re("^[a-z0-9_]{1,64}$", s))
        || has(filters, "stars")
            && !n(&filters["stars"]).is_some_and(|n| n.fract() == 0. && (0. ..=15.).contains(&n))
        || has(filters, "petType")
            && !filters["petType"]
                .as_str()
                .is_some_and(|s| re("^[A-Z0-9_]{1,64}$", s))
    {
        return Err(bad());
    }
    let id = item.to_uppercase();
    if (has(filters, "petType") || has(filters, "petLevel")) && id != "PET" {
        return Err(AppError::new(
            "INVALID_AUCTION_QUERY",
            "Pet filters are supported only for item PET.",
        ));
    }
    if has(filters, "petLevel")
        && !n(&filters["petLevel"]).is_some_and(|n| n.fract() == 0. && (1. ..=200.).contains(&n))
    {
        return Err(AppError::new(
            "INVALID_AUCTION_QUERY",
            "Pet level must be an integer from 1 to 200.",
        ));
    }
    if has(filters, "scrolls") {
        let valid = BLADES.contains(&id.as_str())
            && filters["scrolls"].as_array().is_some_and(|a| {
                a.len() <= 3
                    && a.iter()
                        .all(|v| v.as_str().is_some_and(|s| SCROLLS.contains(&s)))
                    && a.iter()
                        .filter_map(Value::as_str)
                        .collect::<HashSet<_>>()
                        .len()
                        == a.len()
            });
        if !valid {
            return Err(AppError::new(
                "INVALID_AUCTION_QUERY",
                "Use an exact set of supported ability scrolls for a Wither blade.",
            ));
        }
    }
    Ok(id)
}
pub async fn quote(
    u: Arc<Upstream>,
    cache: Arc<Cache>,
    item: &str,
    filters: Value,
) -> Result<Value> {
    let id = validate_filters(item, &filters)?;
    let up = u.clone();
    let snapshot = cache
        .get(
            format!("v7:{}:auction-index", u.source),
            60000,
            180000,
            async move { scan(up, Duration::from_secs(45)).await },
            true,
        )
        .await?;
    let data = &snapshot.value;
    validate(&data["updatedAt"])?;
    let candidates = arr(&data["listings"])
        .into_iter()
        .filter(|l| {
            l[0] == id
                && (!has(&filters, "petType") || l[7] == filters["petType"])
                && n(&l[6]).is_some_and(|n| n > now() as f64)
                && (!has(&filters, "rarity") || l[3] == filters["rarity"])
                && (!has(&filters, "reforge") || l[4] == filters["reforge"])
                && (!has(&filters, "stars") || numeric_eq(&l[5], &filters["stars"]))
        })
        .collect::<Vec<_>>();
    let unknown = candidates
        .iter()
        .filter(|l| {
            has(&filters, "petLevel") && l[8]["petLevel"].is_null()
                || has(&filters, "scrolls") && l[8]["scrolls"].is_null()
        })
        .count();
    let mut scrolls = arr(&filters["scrolls"]);
    scrolls.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    let mut matches = candidates
        .into_iter()
        .filter(|l| {
            (!has(&filters, "petLevel") || numeric_eq(&l[8]["petLevel"], &filters["petLevel"]))
                && (!has(&filters, "scrolls") || l[8]["scrolls"] == json!(scrolls))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|a, b| n(&a[1]).unwrap_or(0.).total_cmp(&n(&b[1]).unwrap_or(0.)));
    let complete = data["complete"] == true && unknown == 0;
    let mut warnings = arr(&data["warnings"])
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    warnings.push("Active BIN asking prices are not completed sales or a valuation of your owned item. Selected scrolls and pet level filters are exact matches, but enchantments, upgrades beyond stars, attributes, skins, pet held items and other variants can differ. A fully scrolled weapon or level-200 pet is not necessarily fully maxed. Listings can be bought or cancelled after the snapshot. Gross SkyBlock coins exclude fees.".into());
    if unknown > 0 {
        warnings.push("Some candidate listings have unreadable variant details; filtered global lowest prices are unknown.".into())
    }
    if data["complete"] != true {
        warnings.push("The scan is incomplete. Only observed prices are available; the global lowest BIN price is unknown.".into())
    }
    if let Some(w) = snapshot.warning {
        warnings.push(w)
    }
    if u.source == "synthetic_demo" {
        warnings.insert(
            0,
            "SYNTHETIC DEMO: this auction snapshot is invented.".into(),
        )
    }
    let mut seen = HashSet::new();
    warnings.retain(|w| seen.insert(w.clone()));
    let lowest = matches.first().and_then(|l| n(&l[1]));
    let mut coverage = json!({"status":if complete{"complete"}else{"partial"},"scanStatus":if data["complete"]==true{"complete"}else{"partial"},"variantStatus":if unknown>0{"partial"}else{"complete"},"unknownVariantListingCount":unknown});
    for key in [
        "scannedPages",
        "totalPages",
        "scannedAuctions",
        "totalAuctions",
        "invalidListings",
        "retryAttempts",
        "recoveredPageFetches",
        "failedPageFetches",
    ] {
        coverage[key] = data[key].clone()
    }
    Ok(
        json!({"metadata":{"source":u.source,"retrievedAt":iso(data["retrievedAt"].integer().unwrap_or(0)),"upstreamUpdatedAt":iso(data["updatedAt"].integer().unwrap_or(0)),"ageSeconds":((now()-data["updatedAt"].integer().unwrap_or(0))/1000).max(0),"cache":snapshot.cache,"stale":snapshot.cache=="stale","freshnessPolicy":{"refreshAfterSeconds":60,"maximumSnapshotAgeSeconds":180,"refreshOnlyOnRequest":true},"warnings":warnings},"item":{"id":id},"filters":filters,"currency":"SkyBlock coins","feesIncluded":false,"priceKind":"active_bin_asking_price","coverage":coverage,"matchedListingCount":matches.len(),"lowestBinCoins":if complete{lowest}else{None},"lowestObservedBinCoins":lowest,"lowestUnitCoins":if complete{matches.iter().filter_map(|l|n(&l[1]).zip(n(&l[2])).map(|(p,c)|p/c)).reduce(f64::min)}else{None},"listings":matches.iter().take(5).map(|l|merge(json!({"coins":l[1],"itemCount":l[2],"unitCoins":n(&l[1]).unwrap()/n(&l[2]).unwrap(),"rarity":l[3],"reforge":l[4],"stars":l[5],"endsAt":iso(n(&l[6]).unwrap_or(0.)as i64),"petType":l[7]}),l[8].clone())).collect::<Vec<_>>()}),
    )
}
fn numeric_eq(a: &Value, b: &Value) -> bool {
    match (n(a), n(b)) {
        (Some(a), Some(b)) => a == b,
        _ => a == b,
    }
}
