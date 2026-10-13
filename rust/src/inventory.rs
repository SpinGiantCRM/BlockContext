use crate::{
    factual::map_fields,
    progression::{MISC, pet_progress},
    types::*,
};
use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    sync::LazyLock,
};
static KNOWN: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fixtures/domain/known-accessories.json")).unwrap()
});
fn mp(rarity: &str) -> Option<f64> {
    match rarity {
        "COMMON" | "SPECIAL" => Some(3.),
        "UNCOMMON" | "VERY SPECIAL" => Some(5.),
        "RARE" => Some(8.),
        "EPIC" => Some(12.),
        "LEGENDARY" => Some(16.),
        "MYTHIC" => Some(22.),
        _ => None,
    }
}
pub fn magical_power(bag: &Value, m: &Value) -> Value {
    let Some(bag) = bag.as_array() else {
        return json!({"currentMagicalPower":null,"calculatedMagicalPower":null,"calculationStatus":"unavailable","reasons":["Accessory bag is unavailable."],"activeItems":null,"suppressedItems":null,"domainRevision":MISC["commit"]});
    };
    let relations = &MISC["data"]["talisman_upgrades"];
    let alias = |id: &str| {
        let mut ids = vec![id.to_owned()];
        ids.extend(
            arr(&relations[id])
                .into_iter()
                .filter(|other| {
                    other
                        .as_str()
                        .is_some_and(|o| arr(&relations[o]).contains(&json!(id)))
                })
                .filter_map(|v| v.as_str().map(str::to_owned)),
        );
        ids.sort();
        ids[0].clone()
    };
    let (mut reasons, mut excluded, mut active) = (vec![], vec![], vec![]);
    let mut groups: Vec<(String, Vec<Value>)> = vec![];
    for item in bag {
        let id = item["id"].as_str();
        let rarity = item["rarity"].as_str();
        if id.is_none() || item["category"] != "ACCESSORY" || rarity.and_then(mp).is_none() {
            reasons.push(format!(
                "Slot {}: accessory identity, category or effective rarity is unknown.",
                item["slot"]
            ));
            continue;
        }
        let id = id.unwrap();
        if !arr(&KNOWN["ids"]).contains(&json!(id)) {
            reasons.push(format!(
                "Slot {}: item {id} is newer than or absent from the supported accessory rules.",
                item["slot"]
            ));
            continue;
        }
        if arr(&MISC["data"]["ignored_talisman"]).contains(&json!(id)) || id == "RIFT_PRISM" {
            excluded.push(json!({"id":id,"slot":item["slot"],"reason":if id=="RIFT_PRISM"{"Prism power requires consumption, not possession."}else{"Rift-only or ignored accessory."}}));
            continue;
        }
        let key = alias(id);
        if let Some((_, items)) = groups.iter_mut().find(|(k, _)| *k == key) {
            items.push(item.clone())
        } else {
            groups.push((key, vec![item.clone()]))
        }
    }
    let mut unique = vec![];
    for (_, mut items) in groups {
        items.sort_by(|a, b| {
            mp(b["rarity"].as_str().unwrap())
                .unwrap()
                .total_cmp(&mp(a["rarity"].as_str().unwrap()).unwrap())
                .then(
                    n(&a["slot"])
                        .unwrap_or(0.)
                        .total_cmp(&n(&b["slot"]).unwrap_or(0.)),
                )
        });
        unique.push(items[0].clone());
        for item in &items[1..] {
            excluded.push(json!({"id":item["id"],"slot":item["slot"],"reason":"Duplicate or alias; the highest effective rarity wins.","replacedBy":items[0]["id"]}))
        }
    }
    for item in &unique {
        let id = item["id"].as_str().unwrap();
        let stronger = unique
            .iter()
            .filter(|other| {
                *other != item
                    && alias(other["id"].as_str().unwrap()) != alias(id)
                    && arr(&relations[id]).contains(&other["id"])
                    && !arr(&relations[other["id"].as_str().unwrap()]).contains(&item["id"])
            })
            .collect::<Vec<_>>();
        if stronger.is_empty() {
            active.push(item.clone())
        } else {
            if stronger.iter().all(|other| {
                mp(other["rarity"].as_str().unwrap()) < mp(item["rarity"].as_str().unwrap())
            }) {
                reasons.push(format!("Upgrade path {id}: lower-tier recombination exceeds the higher tier's rarity; effective precedence needs verification."))
            }
            excluded.push(json!({"id":id,"slot":item["slot"],"reason":"Higher upgrade tier is present in the accessory bag.","replacedBy":stronger.iter().map(|i|&i["id"]).collect::<Vec<_>>()}));
        }
    }
    let contributions=active.iter().map(|i|json!({"id":i["id"],"slot":i["slot"],"rarity":i["rarity"],"magicalPower":mp(i["rarity"].as_str().unwrap()).unwrap()*if i["id"]=="HEGEMONY_ARTIFACT"{2.}else{1.}})).collect::<Vec<_>>();
    let base = contributions
        .iter()
        .filter_map(|v| n(&v["magicalPower"]))
        .sum::<f64>();
    let mut abicase = 0.;
    if active.iter().any(|i| i["id"] == "ABICASE") {
        if let Some(contacts) = at(m, "nether_island_player_data.abiphone.active_contacts")
            .as_array()
            .filter(|a| a.len() <= 256 && a.iter().all(Value::is_string))
        {
            abicase = (contacts
                .iter()
                .filter_map(Value::as_str)
                .collect::<HashSet<_>>()
                .len()
                / 2) as f64
        } else {
            reasons.push("Abicase contact bonus is unavailable.".into())
        }
    }
    let prism = at(m, "rift.access.consumed_prism");
    if has(m, "rift.access.consumed_prism") && !prism.is_boolean() {
        reasons.push("Rift Prism consumption flag is invalid.".into())
    }
    let bonus = if prism == true { 11. } else { 0. };
    let calculated = base + abicase + bonus;
    json!({"currentMagicalPower":if reasons.is_empty(){Some(calculated)}else{None},"calculatedMagicalPower":calculated,"calculationStatus":if reasons.is_empty(){"available"}else{"partial"},"reasons":reasons,"scope":"Accessory bag only, outside Dungeons; carried and stored accessories do not contribute.","breakdown":{"accessories":base,"abicaseContacts":abicase,"consumedRiftPrism":bonus},"activeItems":contributions,"suppressedItems":excluded,"domainRevision":MISC["commit"],"interpretation":"Calculated from this API snapshot and pinned community rules; not a live in-game stat readout. Unknown rarities and bonuses prevent a complete total."})
}
pub fn accessory_diagnostics(bag: &Value, carried: &Value) -> Value {
    let mut groups: Vec<(String, Vec<&Value>)> = vec![];
    let bag_items = arr(bag);
    for i in bag_items.iter() {
        if let Some(id) = i["id"].as_str() {
            if let Some((_, a)) = groups.iter_mut().find(|(k, _)| k == id) {
                a.push(i)
            } else {
                groups.push((id.into(), vec![i]))
            }
        }
    }
    let known = carried.as_array().map(|a| {
        a.iter()
            .filter(|i| i["category"] == "ACCESSORY")
            .cloned()
            .collect::<Vec<_>>()
    });
    json!({"bagItemCount":bag.as_array().map(Vec::len),"distinctBagItemIds":bag.as_array().map(|_|groups.len()),"unknownBagItemIds":bag.as_array().map(|a|a.iter().filter(|i|i["id"].is_null()).count()),"duplicateItemIds":bag.as_array().map(|_|groups.iter().filter(|(_,a)|a.len()>1).map(|(id,a)|json!({"id":id,"occupiedSlots":a.len(),"slots":a.iter().map(|i|&i["slot"]).collect::<Vec<_>>()})).collect::<Vec<_>>()),"carriedAccessories":known,"carriedAccessoriesAbsentFromBag":if bag.is_null(){None}else{known.as_ref().map(|a|a.iter().filter(|i|i["id"].as_str().is_some_and(|id|!groups.iter().any(|(k,_)|k==id))).collect::<Vec<_>>())}})
}
pub const SCROLLS: &[&str] = &[
    "IMPLOSION_SCROLL",
    "SHADOW_WARP_SCROLL",
    "WITHER_SHIELD_SCROLL",
];
pub const BLADES: &[&str] = &["HYPERION", "SCYLLA", "ASTRAEA", "VALKYRIE", "NECRON_BLADE"];
pub fn backpacks(value: &Value, present: bool) -> Value {
    let missing = |warning: &str| json!({"status":"unavailable","items":null,"containers":null,"warnings":[warning]});
    if !present || value.is_null() {
        return missing(
            "Backpack contents are not available. Icons alone do not establish contents or an empty backpack.",
        );
    }
    if !value.is_object() {
        return missing("Backpack contents have an invalid container map.");
    }
    let mut entries = obj(value).into_iter().collect::<Vec<_>>();
    if entries.len() > 36
        || entries
            .iter()
            .any(|(id, _)| !re("^(0|[1-9][0-9]{0,2})$", id))
    {
        return missing("Backpack container identifiers or count exceed supported bounds.");
    }
    entries.sort_by_key(|(id, _)| id.parse::<usize>().unwrap_or(0));
    if entries.is_empty() {
        return json!({"status":"available","items":[],"containers":[],"warnings":[]});
    }
    if entries
        .iter()
        .map(|(_, v)| v["data"].as_str().map(str::len).unwrap_or(0))
        .sum::<usize>()
        > 8 * 1024 * 1024
    {
        return missing("Combined backpack encoding exceeds its size limit.");
    }
    let (mut items, mut containers, mut warnings, mut known) = (vec![], vec![], vec![], 0);
    for (id, v) in entries.iter() {
        let decoded = decode(v).ok().filter(Value::is_array);
        containers.push(json!({"containerId":id,"status":if decoded.is_some(){"available"}else{"unavailable"},"itemCount":decoded.as_ref().and_then(Value::as_array).map(Vec::len)}));
        if let Some(a) = decoded {
            known += 1;
            items.extend(
                arr(&a)
                    .into_iter()
                    .map(|v| merge(v, json!({"containerId":id}))),
            );
        } else {
            warnings.push(format!(
                "Backpack {id}: contents could not be decoded within safety limits."
            ))
        }
    }
    json!({"status":if known==entries.len(){"available"}else if known>0{"partial"}else{"unavailable"},"items":if known>0{json!(items)}else{Value::Null},"containers":containers,"warnings":warnings})
}
pub fn loadouts(v: &Value, present: bool, kind: &str) -> Value {
    let fail = |reason: &str| json!({"status":"unavailable","items":null,"sets":null,"reasonCode":reason,"warnings":[format!("{kind} loadouts: {reason}. Missing contents do not mean empty storage.")]});
    if !present {
        return fail("not_exposed");
    }
    if !v.is_object() {
        return fail("invalid_api_value");
    }
    let mut entries = obj(v)
        .into_iter()
        .filter(|(k, _)| k != "equipped_set")
        .collect::<Vec<_>>();
    if entries.len() > 36 || entries.iter().any(|(k, _)| !re("^(0|[1-9][0-9]{0,2})$", k)) {
        return fail("safety_limit_or_invalid_set_id");
    }
    entries.sort_by_key(|(k, _)| k.parse::<usize>().unwrap_or(0));
    let pieces = if kind == "armor" {
        ["HELMET", "CHESTPLATE", "LEGGINGS", "BOOTS"]
    } else {
        [
            "EQUIPMENT_SLOT_1",
            "EQUIPMENT_SLOT_2",
            "EQUIPMENT_SLOT_3",
            "EQUIPMENT_SLOT_4",
        ]
    };
    let (mut tasks, mut invalid) = (vec![], HashSet::new());
    for (id, v) in &entries {
        if !v.is_object()
            || has(v, "id") && n(&v["id"]) != id.parse::<f64>().ok()
            || obj(v)
                .keys()
                .any(|k| k != "id" && !pieces.contains(&k.as_str()))
        {
            invalid.insert(id.clone());
            continue;
        }
        for piece in pieces {
            if let Some(v) = v.get(piece) {
                tasks.push((id.clone(), piece, v.clone()));
            }
        }
    }
    if tasks
        .iter()
        .map(|(_, _, v)| v["data"].as_str().map(str::len).unwrap_or(0))
        .sum::<usize>()
        > 8 * 1024 * 1024
    {
        return fail("safety_limit");
    }
    let (mut items, mut unknown) = (vec![], HashMap::<String, Vec<&str>>::new());
    for (id, piece, v) in tasks {
        match decode(&v).ok().filter(|v| {
            v.as_array().is_some_and(|a| {
                a.len() <= 1
                    && a.iter()
                        .all(|i| !i["id"].is_null() && n(&i["count"]) == Some(1.))
            })
        }) {
            Some(a) => items.extend(
                arr(&a)
                    .into_iter()
                    .map(|i| merge(i, json!({"setId":id,"piece":piece}))),
            ),
            None => {
                invalid.insert(id.clone());
                unknown.entry(id).or_default().push(piece);
            }
        }
    }
    let status = if invalid.is_empty() {
        "available"
    } else if invalid.len() == entries.len() && items.is_empty() {
        "unavailable"
    } else {
        "partial"
    };
    json!({"status":status,"items":if status=="unavailable"{Value::Null}else{json!(items)},"sets":entries.iter().map(|(id,_)|{let count=items.iter().filter(|i|i["setId"]==*id).count();json!({"setId":id,"status":if invalid.contains(id){"partial"}else{"available"},"itemCount":if invalid.contains(id){None}else{Some(count)},"knownItemCount":count,"unknownPieces":unknown.get(id).cloned().unwrap_or_default(),"equipped":n(&v["equipped_set"])==id.parse::<f64>().ok()})}).collect::<Vec<_>>(),"reasonCode":if invalid.is_empty(){Value::Null}else{json!("decode_or_invalid_api_value")},"warnings":if invalid.is_empty(){vec![]}else{vec![format!("{kind} loadouts contain unreadable pieces; complete item counts are unknown.")]}})
}
pub fn extra_storage(m: &Value, known: &[(String, Value)]) -> Value {
    fn record(
        path: &str,
        items: &Value,
        status: &str,
        reason: Value,
        containers: &mut Vec<Value>,
        records: &mut Vec<Value>,
    ) {
        let view = re("backpack_icons|loadout", path);
        let scope = if path.starts_with("shared_inventory.") {
            "shared_profile"
        } else {
            "member"
        };
        let realm = if path.starts_with("rift.") {
            "rift"
        } else {
            "overworld"
        };
        containers.push(json!({"id":path,"scope":scope,"realm":realm,"viewOnly":view,"status":status,"reasonCode":reason,"returned":items.as_array().map(Vec::len).unwrap_or(0)}));
        records.extend(arr(items).into_iter().map(|i| {
            merge(
                i,
                json!({"location":path,"scope":scope,"realm":realm,"viewOnly":view}),
            )
        }));
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "Bounded recursive projection shares its limits and output accumulators"
    )]
    fn visit(
        v: &Value,
        path: &str,
        depth: usize,
        known: &[(String, Value)],
        scanned: &mut usize,
        truncated: &mut bool,
        containers: &mut Vec<Value>,
        records: &mut Vec<Value>,
    ) {
        if depth > 12 || *scanned >= 64 {
            *truncated = true;
            return;
        }
        if v["data"].is_string() {
            if known.iter().any(|(k, _)| k == path) {
                return;
            }
            *scanned += 1;
            match decode(v) {
                Ok(items) => record(
                    path,
                    &items,
                    if items.is_null() { "unknown" } else { "known" },
                    if items.is_null() {
                        json!("not_exposed")
                    } else {
                        Value::Null
                    },
                    containers,
                    records,
                ),
                Err(_) => record(
                    path,
                    &Value::Null,
                    "unknown",
                    json!("invalid_or_unreadable_encoding"),
                    containers,
                    records,
                ),
            }
            return;
        }
        for (k, v) in obj(v) {
            if re("^[A-Za-z0-9_:.-]{1,96}$", &k) {
                visit(
                    &v,
                    &format!("{path}.{k}"),
                    depth + 1,
                    known,
                    scanned,
                    truncated,
                    containers,
                    records,
                )
            }
        }
    }
    let (mut containers, mut records, mut scanned, mut truncated) = (vec![], vec![], 0, false);
    for (path, items) in known {
        record(
            path,
            items,
            if items.is_null() { "unknown" } else { "known" },
            if items.is_null() {
                json!("not_exposed_or_decode_failed")
            } else {
                Value::Null
            },
            &mut containers,
            &mut records,
        )
    }
    for root in ["inventory", "shared_inventory", "rift", "foraging"] {
        visit(
            &m[root],
            root,
            0,
            known,
            &mut scanned,
            &mut truncated,
            &mut containers,
            &mut records,
        )
    }
    let mut instances: Vec<(String, Vec<&Value>)> = vec![];
    for item in &records {
        if let Some(id) = item["instanceId"].as_str() {
            if let Some((_, a)) = instances.iter_mut().find(|(k, _)| k == id) {
                a.push(item)
            } else {
                instances.push((id.into(), vec![item]))
            }
        }
    }
    let duplicate=instances.iter().filter(|(_,a)|a.len()>1).map(|(id,a)|json!({"instanceId":id,"locations":a.iter().map(|i|json!({"location":i["location"],"slot":i["slot"]})).collect::<Vec<_>>(),"countedAsOneInstance":true})).collect::<Vec<_>>();
    json!({"containers":containers,"items":records,"duplicateViews":duplicate,"uniqueIdentifiedInstances":instances.len(),"unidentifiedRecords":records.iter().filter(|i|i["instanceId"].is_null()).count(),"coverage":{"returnedContainers":containers.len(),"returnedItemRecords":records.len(),"truncated":truncated,"completeOwnedEstate":false,"unviewedPlacedOrEmbeddedItemsUnknown":true,"anonymousDuplicatesUnresolved":true},"materialTotals":null,"reasonCode":"totals_unknown_when_location_or_instance_coverage_is_incomplete"})
}
pub fn digest(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
struct OrderedNbt(Value);
impl<'de> serde::Deserialize<'de> for OrderedNbt {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = OrderedNbt;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "bounded NBT game value")
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<OrderedNbt, E> {
                // Preserve the existing wire representation of NBT TAG_Long:
                // signed high and low words, without JavaScript precision loss.
                Ok(OrderedNbt(json!([(v >> 32) as i32, v as i32])))
            }
            fn visit_i8<E: serde::de::Error>(self, v: i8) -> std::result::Result<OrderedNbt, E> {
                Ok(OrderedNbt(json!(v)))
            }
            fn visit_i16<E: serde::de::Error>(self, v: i16) -> std::result::Result<OrderedNbt, E> {
                Ok(OrderedNbt(json!(v)))
            }
            fn visit_i32<E: serde::de::Error>(self, v: i32) -> std::result::Result<OrderedNbt, E> {
                Ok(OrderedNbt(json!(v)))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<OrderedNbt, E> {
                Ok(OrderedNbt(json!(v)))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<OrderedNbt, E> {
                Ok(OrderedNbt(json!(v)))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<OrderedNbt, E> {
                Ok(OrderedNbt(json!(v)))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<OrderedNbt, E> {
                Ok(OrderedNbt(json!(v)))
            }
            fn visit_bytes<E: serde::de::Error>(
                self,
                v: &[u8],
            ) -> std::result::Result<OrderedNbt, E> {
                Ok(OrderedNbt(json!(v)))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<OrderedNbt, A::Error> {
                let mut out = vec![];
                while let Some(OrderedNbt(v)) = a.next_element()? {
                    out.push(v)
                }
                Ok(OrderedNbt(json!(out)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<OrderedNbt, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some((k, OrderedNbt(v))) = a.next_entry::<String, OrderedNbt>()? {
                    out.insert(k, v);
                }
                for (key, size) in [
                    ("__fastnbt_byte_array", 1),
                    ("__fastnbt_int_array", 4),
                    ("__fastnbt_long_array", 8),
                ] {
                    if out.len() == 1 && out.contains_key(key) {
                        let bytes = arr(&out[key])
                            .iter()
                            .map(|v| v.unsigned().unwrap_or(0) as u8)
                            .collect::<Vec<_>>();
                        if bytes.len() % size != 0 {
                            return Err(serde::de::Error::custom("Invalid NBT array"));
                        }
                        let values = bytes
                            .chunks_exact(size)
                            .map(|c| match size {
                                1 => json!(c[0] as i8),
                                4 => json!(i32::from_be_bytes(c.try_into().unwrap())),
                                _ => json!(i64::from_be_bytes(c.try_into().unwrap())),
                            })
                            .collect::<Vec<_>>();
                        return Ok(OrderedNbt(json!(values)));
                    }
                }
                Ok(OrderedNbt(Value::Object(out)))
            }
        }
        d.deserialize_any(Visitor)
    }
}
// Validate declared NBT sizes before the allocating deserializer sees them.
fn nbt_guard(data: &[u8]) -> std::result::Result<(), ()> {
    struct Scan<'a> {
        b: &'a [u8],
        p: usize,
        n: usize,
    }
    impl Scan<'_> {
        fn take(&mut self, n: usize) -> std::result::Result<&[u8], ()> {
            let end = self.p.checked_add(n).ok_or(())?;
            if end > self.b.len() {
                return Err(());
            }
            let slice = &self.b[self.p..end];
            self.p = end;
            Ok(slice)
        }
        fn byte(&mut self) -> std::result::Result<u8, ()> {
            Ok(self.take(1)?[0])
        }
        fn len(&mut self) -> std::result::Result<usize, ()> {
            let n = i32::from_be_bytes(self.take(4)?.try_into().map_err(|_| ())?);
            if n < 0 {
                return Err(());
            }
            Ok(n as usize)
        }
        fn name(&mut self) -> std::result::Result<(), ()> {
            let n = u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| ())?) as usize;
            self.take(n)?;
            Ok(())
        }
        fn payload(&mut self, kind: u8, depth: usize) -> std::result::Result<(), ()> {
            self.n += 1;
            if self.n > 100000 || depth > 32 {
                return Err(());
            }
            match kind {
                1 => {
                    self.take(1)?;
                }
                2 => {
                    self.take(2)?;
                }
                3 | 5 => {
                    self.take(4)?;
                }
                4 | 6 => {
                    self.take(8)?;
                }
                7 | 11 | 12 => {
                    let n = self.len()?;
                    let size = match kind {
                        11 => 4,
                        12 => 8,
                        _ => 1,
                    };
                    self.take(n.checked_mul(size).ok_or(())?)?;
                }
                8 => self.name()?,
                9 => {
                    let kind = self.byte()?;
                    let n = self.len()?;
                    if n > 100000 || n > self.b.len().saturating_sub(self.p) || kind == 0 && n > 0 {
                        return Err(());
                    }
                    for _ in 0..n {
                        self.payload(kind, depth + 1)?
                    }
                }
                10 => loop {
                    let kind = self.byte()?;
                    if kind == 0 {
                        break;
                    }
                    self.name()?;
                    self.payload(kind, depth + 1)?;
                },
                _ => return Err(()),
            }
            Ok(())
        }
    }
    let mut s = Scan {
        b: data,
        p: 0,
        n: 0,
    };
    if s.byte()? != 10 {
        return Err(());
    }
    s.name()?;
    s.payload(10, 0)?;
    if s.p != data.len() {
        return Err(());
    }
    Ok(())
}
pub fn decode_nbt(data: &str, encoded_limit: usize, expanded_limit: usize) -> Result<Value> {
    let _span = crate::diagnostics::current_span("inventory_decode");
    crate::diagnostics::current_count("inventory_input_bytes", data.len() as u64);
    let result = (|| {
        let error = || {
            AppError::new(
                "INVENTORY_INVALID",
                "The inventory could not be decoded within safety limits.",
            )
        };
        if data.len() > encoded_limit
            || !data.len().is_multiple_of(4)
            || !re("^[A-Za-z0-9+/]*={0,2}$", data)
        {
            return Err(error());
        }
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| error())?;
        if compressed.len() > 2 * 1024 * 1024 {
            return Err(error());
        }
        let gzip = crate::diagnostics::current_span("gzip_expand");
        let mut expanded = Vec::new();
        flate2::read::GzDecoder::new(compressed.as_slice())
            .take(expanded_limit as u64 + 1)
            .read_to_end(&mut expanded)
            .map_err(|_| error())?;
        drop(gzip);
        crate::diagnostics::current_count("inventory_expanded_bytes", expanded.len() as u64);
        if expanded.len() > expanded_limit {
            return Err(error());
        }
        let guard = crate::diagnostics::current_span("nbt_validate");
        nbt_guard(&expanded).map_err(|_| error())?;
        drop(guard);
        let _parse = crate::diagnostics::current_span("nbt_parse");
        let OrderedNbt(v) = fastnbt::from_bytes(&expanded).map_err(|_| error())?;
        Ok(v)
    })();
    if result.is_err() {
        crate::diagnostics::current_count("inventory_decode_failures", 1);
    }
    result
}
pub fn decode(v: &Value) -> Result<Value> {
    if !has(v, "data") {
        return Ok(Value::Null);
    }
    let s = v["data"].as_str().ok_or_else(|| {
        AppError::new(
            "INVENTORY_INVALID",
            "The inventory encoding is invalid or too large.",
        )
    })?;
    project_items(&decode_nbt(s, 3 * 1024 * 1024, 8 * 1024 * 1024)?)
}
pub fn stored_weapon(lore: &Value) -> Value {
    let reports = arr(lore)
        .iter()
        .filter_map(|v| text(v, 160))
        .filter(|s| s.starts_with("Stored Weapon: "))
        .collect::<Vec<_>>();
    if reports.len() != 1 {
        return Value::Null;
    }
    let regex =
        regex::Regex::new(r"^Stored Weapon: ([A-Za-z][A-Za-z '-]{0,95}?)(?: ([✪★]{1,15}))?$")
            .unwrap();
    let Some(m) = regex.captures(&reports[0]) else {
        return Value::Null;
    };
    let name = m.get(1).unwrap().as_str();
    if ["None", "No Weapon"].contains(&name) {
        return json!({"status":"known_empty","displayName":null,"stars":null,"completeWeaponDetails":false});
    }
    json!({"status":"partial","displayName":name.trim(),"stars":m.get(2).map(|v|v.as_str().chars().count()),"id":null,"reforge":null,"enchantments":null,"completeWeaponDetails":false,"reasonCode":"tooltip_identity_and_displayed_stars_only"})
}
pub fn item_details(extra: &Value, lore: &Value) -> Value {
    let mut out = json!({});
    if extra["uuid"]
        .as_str()
        .is_some_and(|s| re("(?i)^[a-f0-9-]{32,36}$", s))
    {
        out["instanceId"] = json!(digest(
            &extra["uuid"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .replace('-', "")
        ))
    }
    let mut upgrades = json!({});
    for key in [
        "hot_potato_count",
        "rarity_upgrades",
        "upgrade_level",
        "dungeon_item_level",
        "art_of_war_count",
        "art_of_peace_count",
        "farming_for_dummies_count",
        "wood_singularity_count",
        "tuned_transmission",
        "mana_disintegrator_count",
        "etherwarp",
        "drill_fuel",
        "drill_fuel_capacity",
        "fuel",
        "pickonimbus_durability",
        "champion_combat_xp",
        "cultivating",
        "farmed_cultivating",
        "compact_blocks",
        "expertise_kills",
        "bass_weight",
        "edition",
    ] {
        if nonnegative(&extra[key]).is_some_and(|n| n <= 1e15) {
            upgrades[key] = extra[key].clone()
        }
    }
    out["upgrades"] = upgrades;
    out["abilityScrolls"] = extra["ability_scroll"]
        .as_array()
        .map(|a| {
            json!(
                a.iter()
                    .filter(|v| v.as_str().is_some_and(|s| re("^[A-Z_]{1,96}$", s)))
                    .take(16)
                    .collect::<Vec<_>>()
            )
        })
        .unwrap_or(Value::Null);
    for key in ["attributes", "runes"] {
        out[key] = if has(extra, key) {
            numeric_map(&extra[key], if key == "attributes" { 64 } else { 32 })
        } else {
            Value::Null
        };
    }
    let mut gems = json!({});
    for (k, v) in obj(&extra["gems"]).into_iter().take(64) {
        if !re("^[A-Z0-9_]{1,64}$", &k) {
            continue;
        }
        if v.as_str().is_some_and(|s| re("^[A-Z0-9_]{1,32}$", s)) {
            gems[&k] = v
        } else if v.is_object() {
            gems[&k] = Value::Object(
                obj(&v)
                    .into_iter()
                    .filter(|(k, v)| {
                        ["quality", "type"].contains(&k.as_str())
                            && v.as_str().is_some_and(|s| re("^[A-Z0-9_]{1,32}$", s))
                    })
                    .collect(),
            )
        }
    }
    out["gems"] = if has(extra, "gems") {
        gems
    } else {
        Value::Null
    };
    for key in [
        "drill_part_engine",
        "drill_part_fuel_tank",
        "drill_part_upgrade_module",
        "power_ability_scroll",
        "skin",
    ] {
        if extra[key]
            .as_str()
            .is_some_and(|s| re("^[A-Za-z0-9_:.-]{1,96}$", s))
        {
            out[key] = extra[key].clone()
        }
    }
    out["soulbound"] = json!(
        extra["donated_museum"]
            .as_bool()
            .or(extra["soulbound"].as_bool())
    );
    out["storedWeapon"] = stored_weapon(lore);
    let mut attrs = extra.clone();
    if let Some(s) = attrs["petInfo"].as_str() {
        let value = serde_json::from_str(s).unwrap_or(Value::Null);
        attrs["petInfo"] = value;
    }
    out["structuredAttributes"] = map_fields(&attrs, "attributes", "item", 256);
    out["receivedAttributeKeys"] = json!(
        obj(&attrs)
            .keys()
            .filter(|k| !re(
                "(?i)(uuid|owner|donator|originator|history|timestamp|secret|token|email)",
                k
            ))
            .take(128)
            .collect::<Vec<_>>()
    );
    let r = regex::Regex::new(r"^([A-Za-z ]{1,40}): ([+-]?[\d,]+(?:\.\d+)?)(%?)(?: |$)").unwrap();
    out["displayedStats"]=lore.as_array().map(|a|json!(a.iter().filter_map(|v|{let line=text(v,256)?;let m=r.captures(&line)?;Some(json!({"label":m.get(1)?.as_str(),"value":m.get(2)?.as_str().replace(',',"").parse::<f64>().ok()?,"unit":if m.get(3)?.as_str()=="%"{"%"}else{"stat"},"valueKind":"primary_displayed_item_stat"}))}).collect::<Vec<_>>())).unwrap_or(Value::Null);
    out["detailCoverage"] =
        json!({"complete":false,"rawLoreExposed":false,"unknownAbsentUpgrades":true});
    out
}
pub fn project_items(v: &Value) -> Result<Value> {
    let a = v["i"]
        .as_array()
        .filter(|a| a.len() <= 4096)
        .ok_or_else(|| {
            AppError::new(
                "INVENTORY_INVALID",
                "The decoded inventory has an invalid slot list.",
            )
        })?;
    let r=regex::Regex::new(r"^(?:a )?(COMMON|UNCOMMON|RARE|EPIC|LEGENDARY|MYTHIC|DIVINE|SPECIAL|VERY SPECIAL) (?:DUNGEON )?([A-Z ]{1,32})(?: a)?$").unwrap();
    Ok(json!(a.iter().enumerate().filter(|(_,v)|!obj(v).is_empty()&&n(&v["Count"])!=Some(0.)).map(|(slot,v)|{let extra=at(v,"tag.ExtraAttributes");let lore=at(v,"tag.display.Lore");let footer=arr(lore).last().and_then(|v|text(v,96));let parsed=footer.as_ref().and_then(|s|r.captures(s));merge(json!({"slot":slot,"id":text(&extra["id"],96),"name":text(at(v,"tag.display.Name"),96),"count":n(&v["Count"]),"reforge":text(&extra["modifier"],96),"enchantments":numeric_map(&extra["enchantments"],128),"rarityUpgrade":n(&extra["rarity_upgrades"]),"dungeonUpgradeLevel":n(&extra["upgrade_level"]).or(n(&extra["dungeon_item_level"])),"rarity":parsed.as_ref().and_then(|c|c.get(1)).map(|m|m.as_str()),"loreCategory":parsed.as_ref().and_then(|c|c.get(2)).map(|m|m.as_str().trim())}),item_details(extra,lore))}).collect::<Vec<_>>()))
}
pub fn describe(items: &Value, catalog: &Value) -> Value {
    items.as_array().map(|a|json!(a.iter().map(|item|{let def=&catalog["items"][item["id"].as_str().unwrap_or("")];let category=text(&def["category"],96);merge(item.clone(),json!({"category":category.clone().map(|s|json!(s)).unwrap_or_else(||item["loreCategory"].clone()),"categorySource":if category.is_some(){json!("hypixel_item_catalog")}else if !item["loreCategory"].is_null(){json!("inventory_footer")}else{Value::Null},"catalogTier":text(&def["tier"],96),"officialDefinition":if obj(def).is_empty(){Value::Null}else{def.clone()},"requirements":def["requirements"],"baseStats":def["stats"],"npcSellPrice":def["npcSellPrice"],"canAuction":def["can_auction"].as_bool(),"canTrade":def["can_trade"].as_bool(),"raritySource":if item["rarity"].is_null(){Value::Null}else{json!("inventory_footer")}}))}).collect::<Vec<_>>())).unwrap_or(Value::Null)
}
pub fn huntaxe(items: &Value) -> Value {
    let reports = arr(items)
        .into_iter()
        .filter(|i| {
            ([
                "VENATOR_GENESIS",
                "SILVA_DOMINUS",
                "CURSUS_FERAE",
                "APEX_PRAEDATOR",
                "NEX_TITANUM",
            ]
            .contains(&i["id"].as_str().unwrap_or(""))
                || !i["storedWeapon"].is_null())
                && !i["storedWeapon"].is_null()
        })
        .collect::<Vec<_>>();
    if reports.is_empty() {
        return json!({"status":"unavailable","reasonCode":"stored_weapon_not_exposed_in_observed_item_lore","weapon":null,"source":"Hypixel profile inventory item lore"});
    }
    let different = reports
        .iter()
        .any(|r| r["storedWeapon"] != reports[0]["storedWeapon"]);
    json!({"status":"partial","reasonCode":if different{"conflicting_tooltip_reports"}else{"tooltip_identity_and_displayed_stars_only"},"weapon":if different{Value::Null}else{reports[0]["storedWeapon"].clone()},"source":"Hypixel profile inventory item lore","tools":reports.iter().map(|i|json!({"id":i["id"],"instanceId":i["instanceId"],"slot":i["slot"],"location":i["location"],"storedWeapon":i["storedWeapon"]})).collect::<Vec<_>>(),"fullEnchantmentAndUpgradeDetailsKnown":false})
}
pub fn variants(id: &str, extra: &Value) -> Value {
    if id != "PET" && !BLADES.contains(&id) {
        return json!({});
    }
    let scrolls = if BLADES.contains(&id) {
        if !has(extra, "ability_scroll") {
            Some(vec![])
        } else if let Some(a) = extra["ability_scroll"].as_array().filter(|a| {
            a.len() <= 3
                && a.iter()
                    .all(|v| v.as_str().is_some_and(|s| SCROLLS.contains(&s)))
        }) {
            let mut a = a.clone();
            a.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            if a.windows(2).any(|w| w[0] == w[1]) {
                None
            } else {
                Some(a)
            }
        } else {
            None
        }
    } else {
        None
    };
    let mut out = json!({"scrolls":scrolls,"scrollCount":scrolls.as_ref().map(Vec::len),"fullyScrolled":scrolls.as_ref().map(|a|a.len()==3)});
    if id == "PET" {
        let pet = extra["petInfo"]
            .as_str()
            .filter(|s| s.len() <= 4096)
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .unwrap_or(json!({}));
        let p = pet_progress(&pet["type"], &pet["tier"], &pet["exp"]);
        out = merge(
            out,
            json!({"petLevel":p["level"],"petMaxLevel":p["maxLevel"],"petExperience":nonnegative(&pet["exp"]),"petRarity":pet["tier"].as_str().filter(|s|re("^[A-Z0-9_]{1,96}$",s)),"petHeldItem":pet["heldItem"].as_str().filter(|s|re("^[A-Z0-9_]{1,96}$",s)),"petSkin":pet["skin"].as_str().filter(|s|re("^[A-Z0-9_]{1,96}$",s)),"petLevelReason":p["reasonCode"]}),
        );
    }
    out
}
