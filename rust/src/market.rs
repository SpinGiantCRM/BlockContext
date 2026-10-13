use crate::{cache::Cache, types::*, upstream::Upstream};
use serde_json::{Value, json};
use std::sync::Arc;
fn book(value: &Value, ascending: bool) -> Option<Vec<Value>> {
    let a = value.as_array().filter(|a| a.len() <= 30)?;
    if a.iter().any(|r| {
        !n(&r["amount"]).is_some_and(|n| n > 0. && n.fract() == 0.)
            || !n(&r["pricePerUnit"]).is_some_and(|n| n > 0.)
    }) {
        return None;
    }
    let mut rows = a
        .iter()
        .map(|r| json!({"amount":r["amount"],"pricePerUnit":r["pricePerUnit"]}))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        let order = n(&a["pricePerUnit"])
            .unwrap()
            .total_cmp(&n(&b["pricePerUnit"]).unwrap());
        if ascending { order } else { order.reverse() }
    });
    Some(rows)
}
pub fn fill(rows: Option<Vec<Value>>, quantity: u64) -> Value {
    let Some(rows) = rows else {
        return json!({"status":"unavailable","bestUnitPrice":null,"totalCoins":null,"filledQuantity":null,"remainingQuantity":quantity,"averageUnitPrice":null,"visibleDepth":null});
    };
    let (mut remaining, mut coins) = (quantity as f64, 0.);
    for row in &rows {
        let take = remaining.min(n(&row["amount"]).unwrap());
        coins += take * n(&row["pricePerUnit"]).unwrap();
        remaining -= take;
        if remaining == 0. {
            break;
        }
    }
    let filled = quantity as f64 - remaining;
    json!({"status":if remaining>0.{"partial"}else{"available"},"bestUnitPrice":rows.first().and_then(|r|n(&r["pricePerUnit"])),"totalCoins":if remaining>0.{None}else{Some(coins)},"visibleFillCoins":if filled>0.{Some(coins)}else{None},"filledQuantity":filled,"remainingQuantity":remaining,"averageUnitPrice":if filled>0.{Some(coins/filled)}else{None},"visibleDepth":rows.iter().filter_map(|r|n(&r["amount"])).sum::<f64>()})
}
pub fn validate_time(v: &Value, t: i64, code: &str, message: &str) -> Result<i64> {
    let timestamp = n(v)
        .filter(|n| *n <= t as f64 + 10000. && t as f64 - *n < 180000.)
        .ok_or_else(|| AppError::retry(code, message, 15))?;
    Ok(timestamp as i64)
}
pub async fn quote(
    upstream: Arc<Upstream>,
    cache: Arc<Cache>,
    item: &str,
    quantity: u64,
) -> Result<Value> {
    if !re("^[a-zA-Z0-9_:.-]{1,96}$", item) || !(1..=1000000).contains(&quantity) {
        return Err(AppError::new(
            "INVALID_MARKET_QUERY",
            "Provide an exact item ID and a whole-number quantity from 1 to 1000000.",
        ));
    }
    let id = item.to_uppercase();
    let prefix = format!("v5:{}", upstream.source);
    let u = upstream.clone();
    let snapshot=cache.get(format!("{prefix}:bazaar"),45000,180000,async move{let data=u.bazaar().await?;let retrieved=now();validate_time(&data["lastUpdated"],retrieved,"MARKET_SNAPSHOT_EXPIRED","The upstream market snapshot is missing a valid recent timestamp. Try again later.")?;Ok(json!({"data":data,"retrievedAt":retrieved}))},true).await?;
    validate_time(
        at(&snapshot.value, "data.lastUpdated"),
        now(),
        "MARKET_SNAPSHOT_EXPIRED",
        "The upstream market snapshot is missing a valid recent timestamp. Try again later.",
    )?;
    let u = upstream.clone();
    let catalog = cache
        .get(
            format!("{prefix}:item-catalog"),
            86400000,
            172800000,
            async move { u.items().await },
            false,
        )
        .await
        .map(|v| v.value)
        .unwrap_or(json!({}));
    let products = at(&snapshot.value, "data.products");
    let product = &products[&id];
    let definition = &catalog["items"][&id];
    if !has(products, &id) && obj(definition).is_empty() {
        return Err(AppError::new(
            "MARKET_ITEM_NOT_FOUND",
            "This item ID is absent from the Bazaar and available item catalog.",
        ));
    }
    let buy = fill(book(&product["buy_summary"], true), quantity);
    let sell = fill(book(&product["sell_summary"], false), quantity);
    let mut warnings=vec!["Quotes use the visible top 30 order levels; they are not guaranteed execution prices. All coin values are gross and exclude taxes and fees. No trading or gameplay action is performed.".to_owned()];
    if let Some(w) = snapshot.warning {
        warnings.push(w)
    }
    if upstream.source == "synthetic_demo" {
        warnings.insert(
            0,
            "SYNTHETIC DEMO: these are invented market prices.".into(),
        )
    }
    if buy["status"] != "available" || sell["status"] != "available" {
        warnings.push(
            "A missing or incomplete order book cannot supply a complete quote for this quantity."
                .into(),
        )
    }
    let npc = n(&definition["npcSellPrice"]);
    let updated = n(at(&snapshot.value, "data.lastUpdated")).unwrap_or(0.) as i64;
    let retrieved = snapshot.value["retrievedAt"]
        .integer()
        .unwrap_or(snapshot.fetched_at);
    let bazaar = if !has(products, &id) {
        json!({"status":"unavailable","reason":"ITEM_NOT_TRADED_ON_BAZAAR","instantBuy":null,"instantSell":null})
    } else {
        json!({"status":if buy["status"]=="available"&&sell["status"]=="available"{"available"}else{"partial"},"instantBuy":buy,"instantSell":sell,"quickStatusWeightedPrices":{"buy":n(at(product,"quick_status.buyPrice")),"sell":n(at(product,"quick_status.sellPrice"))},"volumes":{"buy":n(at(product,"quick_status.buyVolume")),"sell":n(at(product,"quick_status.sellVolume")),"boughtMovingWeek":n(at(product,"quick_status.buyMovingWeek")),"soldMovingWeek":n(at(product,"quick_status.sellMovingWeek"))}})
    };
    Ok(
        json!({"metadata":{"source":upstream.source,"retrievedAt":iso(retrieved),"upstreamUpdatedAt":iso(updated),"ageSeconds":((now()-retrieved)/1000).max(0),"upstreamAgeSeconds":((now()-updated)/1000).max(0),"cache":snapshot.cache,"stale":snapshot.cache=="stale","freshnessPolicy":{"refreshAfterSeconds":45,"maximumSnapshotAgeSeconds":180,"refreshOnlyOnRequest":true},"warnings":warnings},"item":{"id":id,"name":text(&definition["name"],96)},"quantity":quantity,"currency":"SkyBlock coins","feesIncluded":false,"bazaar":bazaar,"npcSell":{"unitCoins":npc,"totalCoins":npc.map(|p|p*quantity as f64),"catalogLastUpdated":n(&catalog["lastUpdated"])},"auctionPrice":null,"auctionCoverage":"Not included; item variants cannot be valued from Bazaar prices."}),
    )
}
