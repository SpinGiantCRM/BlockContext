use crate::{factual::map_fields, progression::pet_progress, types::*};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::LazyLock,
};
pub static SCHEMAS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fixtures/contracts/client-schemas.json")).unwrap()
});
pub fn private_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}
/// A bounded projector for our checked-in JSON contracts. Unknown object keys
/// are stripped, matching the former validator; no remote schemas or code run.
pub fn project_schema(v: &Value, s: &Value, depth: usize) -> Option<Value> {
    if depth > 64 {
        return None;
    }
    if let Some(any) = s["anyOf"].as_array() {
        return any.iter().find_map(|s| project_schema(v, s, depth + 1));
    }
    if let Some(one) = s["oneOf"].as_array() {
        let matches = one
            .iter()
            .filter_map(|s| project_schema(v, s, depth + 1))
            .collect::<Vec<_>>();
        return (matches.len() == 1).then(|| matches[0].clone());
    }
    if has(s, "const") && !numeric_equal(v, &s["const"]) {
        return None;
    }
    if let Some(a) = s["enum"].as_array()
        && !a.iter().any(|e| numeric_equal(v, e))
    {
        return None;
    }
    if let Some(kind) = s["type"].as_str()
        && !match kind {
            "object" => v.is_object(),
            "array" => v.is_array(),
            "string" => v.is_string(),
            "number" => v.is_number(),
            "integer" => n(v).is_some_and(|n| n.fract() == 0.),
            "boolean" => v.is_boolean(),
            "null" => v.is_null(),
            _ => false,
        }
    {
        return None;
    }
    if let Some(n) = n(v)
        && (s["minimum"].as_f64().is_some_and(|min| n < min)
            || s["maximum"].as_f64().is_some_and(|max| n > max)
            || s["exclusiveMinimum"].as_f64().is_some_and(|min| n <= min)
            || s["exclusiveMaximum"].as_f64().is_some_and(|max| n >= max))
    {
        return None;
    }
    if let Some(text) = v.as_str() {
        let len = text.chars().count();
        if s["minLength"]
            .unsigned()
            .is_some_and(|min| len < (min as usize))
            || s["maxLength"]
                .unsigned()
                .is_some_and(|max| len > (max as usize))
            || s["pattern"].as_str().is_some_and(|p| !re(p, text))
        {
            return None;
        }
    }
    if let Some(a) = v.as_array() {
        if s["minItems"]
            .unsigned()
            .is_some_and(|min| a.len() < min as usize)
            || s["maxItems"]
                .unsigned()
                .is_some_and(|max| a.len() > max as usize)
        {
            return None;
        }
        return Some(Value::Array(
            a.iter()
                .map(|v| {
                    if has(s, "items") {
                        project_schema(v, &s["items"], depth + 1)
                    } else {
                        Some(v.clone())
                    }
                })
                .collect::<Option<_>>()?,
        ));
    }
    if let Some(o) = v.as_object() {
        if s["required"].as_array().is_some_and(|a| {
            a.iter()
                .any(|k| k.as_str().is_none_or(|k| !o.contains_key(k)))
        }) {
            return None;
        }
        let mut out = serde_json::Map::new();
        // Typed objects follow contract order, so evidence pagination remains
        // stable across the TypeScript and Rust implementations. Records retain
        // the source order below.
        if let Some(properties) = s["properties"].as_object() {
            for (k, schema) in properties {
                if let Some(value) = o.get(k) {
                    out.insert(k.clone(), project_schema(value, schema, depth + 1)?);
                }
            }
        }
        for (k, v) in o {
            if has(s, "propertyNames")
                && project_schema(&json!(k), &s["propertyNames"], depth + 1).is_none()
            {
                return None;
            }
            if s["properties"].get(k).is_some() {
                continue;
            } else if s["additionalProperties"].is_object() {
                out.insert(
                    k.clone(),
                    project_schema(v, &s["additionalProperties"], depth + 1)?,
                );
            } else if s["additionalProperties"] != false {
                out.insert(k.clone(), v.clone());
            }
        }
        return Some(Value::Object(out));
    }
    Some(v.clone())
}
fn numeric_equal(a: &Value, b: &Value) -> bool {
    match (n(a), n(b)) {
        (Some(a), Some(b)) => a == b,
        _ => a == b,
    }
}
fn safe_attribute(s: &str) -> bool {
    s.split('.').all(|k|re("^[A-Za-z0-9_:-]{1,64}$",k)&&!re("(?i)(?:^|_)(?:uuid|uniqueid|player_id|player_uuid|owner|owners|donator|originator|email|ip|token|secret|password|session|chat|message|history)(?:$|_)",k)&&uuid(k).is_none())
}
fn safe_item(v: &Value) -> bool {
    if arr(at(v, "structuredAttributes.fields")).iter().any(|f| {
        f["path"].as_str().is_none_or(|s| !safe_attribute(s))
            || f["value"].as_str().is_some_and(|s| uuid(s).is_some())
    }) || arr(&v["receivedAttributeKeys"])
        .iter()
        .any(|k| k.as_str().is_none_or(|s| !safe_attribute(s)))
    {
        return false;
    }
    for (k, max) in [
        ("enchantments", 128),
        ("attributes", 64),
        ("runes", 64),
        ("gems", 64),
    ] {
        if obj(&v[k]).len() > max {
            return false;
        }
    }
    true
}
fn normalized_item(i: &Value) -> Value {
    let fields = arr(at(i, "structuredAttributes.fields"));
    let attributes = fields
        .iter()
        .filter_map(|f| {
            f["path"]
                .as_str()
                .map(|k| (k.to_owned(), f["value"].clone()))
        })
        .collect::<serde_json::Map<_, _>>();
    let mut out = merge(
        i.clone(),
        json!({"name":i["displayName"],"rarityUpgrade":i["rarity_upgrades"],"dungeonUpgradeLevel":i.get("upgrade_level").or(i.get("dungeon_item_level")).unwrap_or(&Value::Null),"soulbound":attributes.get("soulbound").and_then(Value::as_bool).or_else(||attributes.get("donated_museum").and_then(Value::as_bool)),"upgrades":Value::Object(obj(i).into_iter().filter(|(k,v)|v.is_number()&&!["slot","count"].contains(&k.as_str())).collect())}),
    );
    if i["id"] == "PET" {
        let get = |k: &str| attributes.get(k).cloned().unwrap_or(Value::Null);
        out["pet"] = json!({"type":get("petInfo.type"),"tier":get("petInfo.tier"),"experience":get("petInfo.exp"),"heldItem":get("petInfo.heldItem"),"candiesUsed":get("petInfo.candyUsed"),"skin":get("petInfo.skin"),"progression":pet_progress(&get("petInfo.type"),&get("petInfo.tier"),&get("petInfo.exp"))});
    }
    out
}
pub fn private_read(path: &Path, max: usize) -> std::io::Result<Vec<u8>> {
    let _span = crate::diagnostics::current_span("client_file_read");
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    let info = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if info.permissions().mode() & 0o077 != 0 {
            return Err(std::io::Error::other("Private file required"));
        }
    }
    if !info.is_file() || info.len() > max as u64 {
        return Err(std::io::Error::other("Bounded regular file required"));
    }
    let mut blob = Vec::new();
    file.by_ref().take(max as u64 + 1).read_to_end(&mut blob)?;
    if blob.len() > max {
        return Err(std::io::Error::other("File limit"));
    }
    crate::diagnostics::current_count("client_snapshot_bytes", blob.len() as u64);
    Ok(blob)
}
pub fn validate_snapshot(v: &Value) -> Option<Value> {
    let _span = crate::diagnostics::current_span("client_validate");
    let v = project_schema(v, &SCHEMAS["snapshot"], 0)?;
    for p in arr(&v["profiles"]) {
        if obj(&p["observations"]).len() > 96 {
            return None;
        }
        for c in arr(&p["changes"]) {
            if n(&c["previousObservedAt"])? > n(&c["observedAt"])?
                || c["path"]
                    .as_str()
                    .is_none_or(|s| re("(?i)(uuid|token|secret|password|email|chat|message)", s))
            {
                return None;
            }
        }
    }
    Some(v)
}
fn ledger_valid(v: &Value, t: i64) -> bool {
    let get = |k: &str| v[k].integer().unwrap_or(-1);
    let has_pair = |a: &str, b: &str| has(v, a) == has(v, b);
    get("persistentObservedBonusClaims") <= get("persistentObservedClaims")
        && get("observedBonusClaimsInReferenceDay") <= get("persistentObservedBonusClaims")
        && get("expectedNextResetAt") == get("referenceDayStart") + 86400000
        && has_pair("lastObservedRemaining", "lastRemainingObservedAt")
        && has_pair("bonusRenewalAfter", "bonusRenewalObservedAt")
        && (!has(v, "bonusRenewalAfter")
            || get("bonusRenewalObservedAt") >= get("bonusRenewalAfter")
                && get("resetObservationWindowMilliseconds")
                    == get("bonusRenewalObservedAt") - get("bonusRenewalAfter"))
        && [
            "ledgerCreatedAt",
            "referenceDayStart",
            "lastRemainingObservedAt",
            "bonusRenewalObservedAt",
        ]
        .iter()
        .all(|k| !has(v, k) || get(k) <= t)
}
fn commissions_valid(v: &Value, t: i64, read: &str) -> bool {
    let count = v["remainingBonusCommissions"].integer();
    let bonus = v["dailyBonusAvailable"].as_bool();
    !obj(v).is_empty()
        && !(count == Some(0) && bonus == Some(true))
        && !(bonus == Some(false)
            && (count != Some(0)
                || v["availabilityEvidence"] != "validated_current_reward_previews"))
        && ((!has(v, "observedClaimTransitions") && !has(v, "observedBonusClaimTransitions"))
            || (has(v, "observedClaimTransitions")
                && has(v, "observedBonusClaimTransitions")
                && has(v, "claimCountScope")))
        && (!has(v, "observedClaimTransitions")
            || v["observedBonusClaimTransitions"].integer()
                <= v["observedClaimTransitions"].integer())
        && (has(v, "bonusRenewalAfter") == has(v, "bonusRenewalObservedAt"))
        && (!has(v, "bonusRenewalObservedAt")
            || v["bonusRenewalAfter"].integer() <= v["bonusRenewalObservedAt"].integer()
                && v["bonusRenewalObservedAt"]
                    .integer()
                    .is_some_and(|n| n <= t))
        && (!has(v, "availabilityEvidence") || read == "opened_menu")
}
pub fn validate_contract(name: &str, v: &Value) -> Option<Value> {
    if name == "snapshot" {
        return validate_snapshot(v);
    }
    let v = project_schema(v, &SCHEMAS[name], 0)?;
    match name {
        "item" => {
            if !safe_item(&v) {
                return None;
            }
        }
        "commissionTracking" => {
            if !ledger_valid(&v, 1000000000000000) {
                return None;
            }
        }
        "commissions" => {
            if !commissions_valid(&v, 9007199254740991, "opened_menu") {
                return None;
            }
        }
        "commissionEvidence" => {
            if arr(&v["entries"]).iter().any(|e| {
                !e["slot"].integer().is_some_and(|n| (9..18).contains(&n))
                    && e["displayName"] != "Commission Milestones"
            }) {
                return None;
            }
        }
        "change"
            if (n(&v["previousObservedAt"])? > n(&v["observedAt"])?
                || v["path"].as_str().is_none_or(|s| {
                    re("(?i)(uuid|token|secret|password|email|chat|message)", s)
                })) =>
        {
            return None;
        }
        _ => {}
    }
    if arr(&v["items"]).iter().any(|i| !safe_item(i)) {
        return None;
    }
    Some(v)
}
pub fn parse_observation(key: &str, record: &Value) -> Option<(String, Value)> {
    let values = &record["values"];
    let read = record["readFrom"].as_str()?;
    let t = record["observedAt"].integer()?;
    if read == "collector_ledger" && key != "commissionTracking" {
        return None;
    }
    let (group, schema) = match key {
        "cookie" | "godPotion" => ("stats", "buff"),
        "commissions" => ("mining", "commissions"),
        "commissionTracking" => ("mining", "commissionTracking"),
        "commissionMenuEvidence" => ("mining", "commissionEvidence"),
        "inventory" | "toolkit" => ("equipment", "observedItems"),
        "minion" => ("minions", "observedItems"),
        "garden" => ("garden", "observedItems"),
        "sidebar" | "actionBar" | "tabStats" => ("stats", "facts"),
        "mining" | "foraging" | "hunting" | "dailies" | "stats" | "pets" | "collections"
        | "accessories" | "quests" | "museum" | "orders" | "travel" | "fishing" | "bestiary"
        | "dungeons" | "slayers" | "events" => (key, "facts"),
        _ => {
            if let Some(k) = key.strip_prefix("bank:").filter(|k| {
                [
                    "personalBankCoins",
                    "sharedProfileBankCoins",
                    "bankCoinsUnclassified",
                ]
                .contains(k)
            }) {
                let n = nonnegative(&values[k]).filter(|n| *n <= 1e15)?;
                return Some(("currencies".into(), json!({k:n})));
            }
            if key == "huntaxe" {
                let weapon = project_schema(&values["weapon"], &SCHEMAS["item"], 0)?;
                if !safe_item(&weapon) {
                    return None;
                }
                return Some(("equipment".into(), json!({"weapon":weapon})));
            }
            if re(
                r"^storage:(?:(?:Small |Medium |Large |Greater |Jumbo )?Backpack(?: \(Slot #\d{1,2}\))?|Ender Chest(?: \(\d+/\d+\))?|Storage \(\d+/\d+\))$",
                key,
            ) {
                ("equipment", "observedItems")
            } else if key == "liveState" {
                let schema = json!({"type":"object","required":["selectedSlot","minecraftHealth","minecraftMaxHealth","minecraftExperienceLevel","minecraftExperienceProgress","dungeonHudObserved","minecraftEffects"],"properties":{"selectedSlot":{"type":"integer","minimum":0,"maximum":8},"minecraftHealth":{"type":"number","minimum":0},"minecraftMaxHealth":{"type":"number","minimum":0},"minecraftExperienceLevel":{"type":"integer","minimum":0},"minecraftExperienceProgress":{"type":"number","minimum":0,"maximum":1},"dungeonHudObserved":{"type":"boolean"},"minecraftEffects":{"type":"array","maxItems":64,"items":{"type":"object","required":["id","amplifier","remainingTicksAtObservation","infinite","ambient","visible"],"properties":{"id":{"type":"string","pattern":"^[a-z0-9_.:-]{1,96}$"},"amplifier":{"type":"integer","minimum":0,"maximum":255},"remainingTicksAtObservation":{"type":"integer","minimum":-1,"maximum":1000000000},"infinite":{"type":"boolean"},"ambient":{"type":"boolean"},"visible":{"type":"boolean"}},"additionalProperties":false}}},"additionalProperties":false});
                return Some(("stats".into(), project_schema(values, &schema, 0)?));
            } else {
                return None;
            }
        }
    };
    let value = project_schema(values, &SCHEMAS[schema], 0)?;
    match schema {
        "commissionTracking" => {
            if read != "collector_ledger" || !ledger_valid(&value, t) {
                return None;
            }
        }
        "commissions" => {
            if !commissions_valid(&value, t, read) {
                return None;
            }
        }
        "commissionEvidence"
            if (read != "opened_menu"
                || arr(&value["entries"]).iter().any(|e| {
                    !e["slot"].integer().is_some_and(|n| (9..18).contains(&n))
                        && e["displayName"] != "Commission Milestones"
                })) =>
        {
            return None;
        }
        _ => {}
    }
    if arr(&value["items"]).iter().any(|i| !safe_item(i)) {
        return None;
    }
    Some((group.into(), value))
}
#[derive(Clone, Default)]
pub struct Client {
    pub path: Option<PathBuf>,
}
impl Client {
    #[allow(
        clippy::too_many_arguments,
        reason = "Preserve the existing collector query contract and injectable observation clock"
    )]
    pub fn query(
        &self,
        player: &str,
        profile: Option<&str>,
        section: Option<&str>,
        live: bool,
        max_age: i64,
        offset: usize,
        limit: usize,
        t: i64,
    ) -> Result<Value> {
        validate_player(player)?;
        if profile.is_some_and(|p| !re("^[A-Za-z][A-Za-z ]{0,31}$", p)) {
            return Err(AppError::new(
                "INVALID_PROFILE",
                "Provide the exact profile name visible in the game.",
            ));
        }
        if section.is_some_and(|s| !SECTIONS.contains(&s)) {
            return Err(AppError::new(
                "INVALID_SECTION",
                "Choose a supported factual profile section.",
            ));
        }
        if !(2..=300).contains(&max_age) || offset > 50000 || !(1..=100).contains(&limit) {
            return Err(AppError::new(
                "INVALID_LIVE_OPTIONS",
                "Use maximum age 2–300 seconds and bounded pagination.",
            ));
        }
        let unavailable = |reason: &str| json!({"source":"client_observation","status":"unavailable","reasonCode":reason,"observations":{},"apiRequests":0,"mode":if live{"live"}else{"latest_observations"},"warnings":["Client observations are distinct from Hypixel API data. Missing observations do not mean zero or empty storage."]});
        let Some(path) = self.path.as_ref() else {
            return Ok(unavailable("collector_not_configured"));
        };
        let Some(raw) = private_read(path, 1024 * 1024)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| validate_snapshot(&v))
        else {
            return Ok(unavailable("missing_or_invalid_private_snapshot"));
        };
        let exported = raw["exportedAt"].integer().unwrap_or(0);
        if exported > t + 5000 || t - exported > 365 * 86400000 {
            return Ok(unavailable("snapshot_expired_or_future"));
        }
        let profiles = arr(&raw["profiles"]);
        let mut matches = profiles
            .iter()
            .filter(|p| {
                p["uuid"].as_str() == uuid(player).as_deref()
                    || p["playerName"]
                        .as_str()
                        .is_some_and(|n| n.eq_ignore_ascii_case(player))
            })
            .filter(|p| {
                profile.is_none_or(|n| {
                    p["profileName"]
                        .as_str()
                        .is_some_and(|p| p.eq_ignore_ascii_case(n))
                })
            })
            .collect::<Vec<_>>();
        if profile.is_none() && matches.len() > 1 {
            matches.retain(|p| {
                raw["activeProfile"]
                    == format!(
                        "{}:{}",
                        p["uuid"].as_str().unwrap(),
                        p["profileName"].as_str().unwrap().to_lowercase()
                    )
            })
        }
        if matches.len() > 1 {
            return Err(AppError::new(
                "PROFILE_REQUIRED",
                "Choose an exact profile name from your collected game data.",
            ));
        }
        let Some(p) = matches.first() else {
            return Ok(unavailable("no_matching_player_and_profile"));
        };
        let active = format!(
            "{}:{}",
            p["uuid"].as_str().unwrap(),
            p["profileName"].as_str().unwrap().to_lowercase()
        );
        let connected = raw["activeProfile"] == active && t - exported <= max_age * 1000;
        if live && !connected {
            return Ok(merge(
                unavailable("collector_not_connected_or_export_too_old"),
                json!({"exportedAt":iso(exported)}),
            ));
        }
        let (mut out, mut unavailable_obs, mut pagination, mut field_inputs) =
            (json!({}), json!({}), json!({}), json!({}));
        for (key, record) in obj(&p["observations"]) {
            let observed = record["observedAt"].integer().unwrap_or(0);
            let age = t - observed;
            if age < 0
                || age
                    >= if key == "commissionTracking" {
                        365 * 86400000
                    } else {
                        86400000
                    }
                || t - exported >= 86400000 && key != "commissionTracking"
            {
                continue;
            }
            let Some((group, mut values)) = parse_observation(&key, &record) else {
                continue;
            };
            if section.is_some_and(|s| {
                s != "api_fields"
                    && s != "systems"
                    && s != group
                    && !(s == "dailies"
                        && [
                            "commissions",
                            "commissionMenuEvidence",
                            "commissionTracking",
                        ]
                        .contains(&key.as_str()))
                    && !(s == "foraging" && key == "tabStats")
                    && !(s == "storage" && group == "equipment")
            }) {
                continue;
            }
            let observed_now = raw["observedNow"]
                .as_array()
                .map(|a| a.contains(&json!(key)))
                .unwrap_or(key == "inventory");
            let tracker = connected && age <= max_age * 1000 && observed_now;
            let current = key != "commissionTracking" && tracker;
            if live && !tracker {
                unavailable_obs[&key] = json!({"reasonCode":"not_currently_observed_or_too_old","ageSeconds":age/1000,"readFrom":record["readFrom"]});
                continue;
            }
            if let Some(items) = values["items"].as_array() {
                values["items"]=json!(items.iter().map(|i|{let slot=n(&i["slot"]).unwrap_or(-1.);merge(normalized_item(i),json!({"location":if key=="inventory"{"inventory"}else{&key},"equippedState":if key=="inventory"{json!(if (36. ..=39.).contains(&slot){"minecraft_armor_slot"}else if slot==40.{"minecraft_offhand"}else{"carried"})}else{Value::Null}}))}).collect::<Vec<_>>());
            }
            if values["weapon"].is_object() {
                values["weapon"] = normalized_item(&values["weapon"])
            }
            field_inputs[&key] = values.clone();
            for name in ["items", "facts", "entries"] {
                if let Some(a) = values[name].as_array() {
                    let total = a.len();
                    let slice = a
                        .iter()
                        .skip(offset)
                        .take(limit)
                        .cloned()
                        .collect::<Vec<_>>();
                    pagination[format!("{key}.{name}")] = json!({"offset":offset,"limit":limit,"total":total,"returned":slice.len(),"hasMore":offset+limit<total});
                    values[name] = json!(slice);
                }
            }
            let mut row = json!({"source":"client_observation","status":if age>300000{"stale"}else{"available"},"observedAt":iso(observed),"ageSeconds":age/1000,"readFrom":record["readFrom"],"values":values,"coverage":if key=="huntaxe"||key.starts_with("bank:")||group=="stats"{"only_the_displayed_field"}else if key=="inventory"{"only_current_player_inventory_slots"}else{"partial_only_viewed_menu"},"currentStateVerified":current,"scope":"member"});
            if key == "commissionTracking" {
                row["currentTrackerVerified"] = json!(tracker);
                row["coverage"] = json!("persistent_partial_claim_ledger");
            }
            out[&key] = row;
        }
        if t - exported >= 86400000 && !has(&out, "commissionTracking") {
            return Ok(unavailable("snapshot_expired_or_future"));
        }
        let (mut fields, mut coverage) = (vec![], json!({}));
        for (key, row) in obj(&out) {
            let mapped = map_fields(
                &field_inputs[&key],
                &format!("client.{key}"),
                "member",
                12000_usize.saturating_sub(fields.len()),
            );
            coverage[&key] = mapped["coverage"].clone();
            fields.extend(arr(&mapped["fields"]).into_iter().map(|f|{let name=f["path"].as_str().unwrap_or("").rsplit('.').next().unwrap_or("");let canonical=if key.starts_with("bank:"){match name{"personalBankCoins"=>Some("member.profile.bank_account"),"sharedProfileBankCoins"=>Some("profile.banking.balance"),_=>None}}else{None};merge(f,json!({"source":"client_observation","observedAt":row["observedAt"],"currentStateVerified":row["currentStateVerified"],"canonicalApiPath":canonical}))}));
        }
        let total = fields.len();
        let fields = fields
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        pagination["fields"] = json!({"offset":offset,"limit":limit,"total":total,"returned":fields.len(),"hasMore":offset+limit<total});
        Ok(
            json!({"changesSincePreviousObservations":arr(&p["changes"]).into_iter().filter(|c|c["observedAt"].integer().is_some_and(|n|n<=t&&t-n<86400000)).collect::<Vec<_>>(),"changeCoverage":{"maximum":64,"source":"passive_owner_observations","persistentHistory":false,"removedItemsDoNotProveSale":true},"fields":fields,"fieldCoverage":coverage,"source":"client_observation","status":if obj(&out).is_empty(){"unavailable"}else{"partial"},"reasonCode":if obj(&out).is_empty(){"no_valid_recent_observations"}else{"owner_visible_observations"},"player":{"uuid":p["uuid"],"name":p["playerName"]},"profile":{"name":p["profileName"]},"mode":if live{"live"}else{"latest_observations"},"apiRequests":0,"maxAgeSeconds":max_age,"unavailableObservations":unavailable_obs,"pagination":pagination,"collectorCoverage":{"evictedObservations":raw["evictedObservations"].unsigned().unwrap_or(0),"exportIntervalMs":raw["exportIntervalMs"].unsigned().unwrap_or(5000),"onlyReceivedVisibleState":true,"completeProfile":false},"collectorVersion":raw["collectorVersion"],"exportedAt":iso(exported),"connectedAtLastExport":raw["activeProfile"]==active&&t-exported<45000,"observations":out,"warnings":["Owner-only client observations; they are not Hypixel API responses or independently verified values.","Timers are remaining time when observed, not a live countdown. Gameplay can pause effects.","Current inventory, owner state, selected item attributes and displayed HUD/tab/sidebar facts are passive. Server-unsent fields still require a viewed menu or remain unknown. Unviewed storage and minions remain unknown. Ordinary game observations expire after 24 hours; the latest partial commission ledger remains available as dated evidence for up to 365 days. Each key retains only its latest observation."]}),
        )
    }
}
