use crate::{
    factual::{additional_sections, game_evidence},
    inventory as inv, progression as p, systems,
    types::*,
};
use serde_json::{Value, json};
fn section(data: Value, present: bool, mut warnings: Vec<String>) -> Value {
    if !present {
        warnings.insert(
            0,
            "Not available in the API response; this does not mean zero.".into(),
        )
    }
    json!({"status":if !present{"unavailable"}else if warnings.is_empty(){"available"}else{"partial"},"data":data,"warnings":warnings})
}
fn inventory(m: &Value, paths: &[&str]) -> Value {
    for path in paths {
        let path = format!("inventory.{path}");
        if !has(m, &path) {
            continue;
        }
        let value = at(m, &path);
        if !has(value, "data") {
            return json!({"items":null,"reasonCode":"invalid_api_value","warnings":["Inventory field is present but its encoding is missing or invalid."]});
        }
        return match inv::decode(value) {
            Ok(items) => json!({"items":items,"warnings":[],"reasonCode":null}),
            Err(_) => {
                json!({"items":null,"reasonCode":"decode_or_safety_limit","warnings":["Inventory data was present but could not be decoded safely."]})
            }
        };
    }
    json!({"items":null,"reasonCode":"not_exposed","warnings":["Inventory data not present; it may be disabled or unavailable."]})
}
fn warnings(v: &Value) -> Vec<String> {
    arr(&v["warnings"])
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}
pub fn normalize(
    raw: &Value,
    owner: &str,
    catalog: &Value,
    items: &Value,
    metadata: &Value,
    collections: &Value,
) -> Result<Value> {
    let id = raw["profile_id"].as_str().and_then(uuid).ok_or_else(|| {
        AppError::new(
            "UPSTREAM_INVALID",
            "The requested profile or membership is invalid.",
        )
    })?;
    let m = &raw["members"][owner];
    if obj(m).is_empty() {
        return Err(AppError::new(
            "UPSTREAM_INVALID",
            "The requested profile or membership is invalid.",
        ));
    }
    let xp = nonnegative(at(m, "leveling.experience"));
    let fairy = nonnegative(at(m, "fairy_soul.total_collected"));
    let mut stats = section(
        json!({"skyBlockExperience":xp,"skyBlockLevel":xp.map(|x|(x/100.).floor()),"experienceToNextSkyBlockLevel":xp.map(|x|100.-x%100.),"fairySoulsCollected":fairy,"fieldCoverage":{"level":{"status":if xp.is_some(){"available"}else{"unavailable"},"reasonCode":if xp.is_some(){Value::Null}else{json!("not_exposed_or_invalid")}},"fairySouls":{"status":if fairy.is_some(){"available"}else{"unavailable"},"reasonCode":if fairy.is_some(){Value::Null}else{json!("not_exposed_or_invalid")}}}}),
        xp.is_some() || fairy.is_some(),
        if xp.is_none() || fairy.is_none() {
            vec!["Some SkyBlock level or fairy soul fields are unavailable or invalid.".into()]
        } else {
            vec![]
        },
    );
    let buff = p::buffs(m);
    let coverage = merge(
        stats["data"]["fieldCoverage"].clone(),
        buff["fieldCoverage"].clone(),
    );
    stats["data"] = merge(stats["data"].clone(), buff["data"].clone());
    stats["data"]["fieldCoverage"] = coverage;
    stats["status"] = json!(
        if stats["status"] == "unavailable" && buff["anyAvailable"] != true {
            "unavailable"
        } else if stats["status"] != "available" || buff["incomplete"] == true {
            "partial"
        } else {
            "available"
        }
    );
    stats["warnings"]
        .as_array_mut()
        .unwrap()
        .extend(arr(&buff["warnings"]));
    let mut money = json!({});
    let mut money_coverage = json!({});
    for (key, v, present, source) in [
        (
            "purseCoins",
            at(m, "currencies.coin_purse"),
            has(m, "currencies.coin_purse"),
            "member.currencies.coin_purse",
        ),
        (
            "sharedProfileBankCoins",
            at(raw, "banking.balance"),
            has(raw, "banking.balance"),
            "profile.banking.balance",
        ),
        (
            "personalBankCoins",
            at(m, "profile.bank_account"),
            has(m, "profile.bank_account"),
            "member.profile.bank_account",
        ),
        (
            "motes",
            at(m, "currencies.motes_purse"),
            has(m, "currencies.motes_purse"),
            "member.currencies.motes_purse",
        ),
    ] {
        let n = nonnegative(v);
        money[key] = json!(n);
        money_coverage[key] = json!({"status":if n.is_some(){"available"}else{"unavailable"},"reasonCode":if !present{json!("not_exposed")}else if n.is_none(){json!("invalid_api_value")}else{Value::Null},"source":source});
    }
    let money_present = obj(&money).values().any(|v| !v.is_null());
    let money_incomplete = obj(&money_coverage)
        .values()
        .any(|v| v["status"] != "available");
    money = merge(
        money,
        json!({"fieldCoverage":money_coverage,"reportedCurrencies":game_evidence(&m["currencies"]),"bits":game_evidence(at(m,"player_data.bits")),"fame":game_evidence(at(m,"player_data.fame")),"copper":n(at(m,"garden_player_data.copper")),"committedCoins":null,"netWorth":null,"valuationReasonCode":"storage_and_variant_valuations_incomplete"}),
    );
    let currencies = section(
        money,
        money_present,
        if money_incomplete {
            vec!["Some balances are not exposed or invalid; they remain unknown. Personal and shared profile bank balances are separate. Missing API data alone does not establish whether a privacy setting is disabled.".into()]
        } else {
            vec![]
        },
    );
    let slayers = section(
        json!({"bosses":p::slayers(m),"activeQuest":game_evidence(at(m,"slayer.slayer_quest"))}),
        has(m, "slayer.slayer_bosses"),
        vec![],
    );
    let mut pets = p::pets_section(at(m, "pets_data.pets"), has(m, "pets_data.pets"));
    pets["data"]["autopetEvidence"] = game_evidence(at(m, "pets_data.autopet"));
    pets["data"]["petSystemEvidence"] = game_evidence(&Value::Object(
        obj(&m["pets_data"])
            .into_iter()
            .filter(|(k, _)| k != "pets")
            .collect(),
    ));
    let generators = m["player_data"]
        .get("crafted_generators")
        .filter(|v| !v.is_null())
        .unwrap_or(&m["crafted_generators"]);
    let minions = section(
        json!({"craftedGenerators":generators.as_array().map(|a|a.iter().take(2048).filter_map(|v|text(v,96)).filter(|s|!s.is_empty()).collect::<Vec<_>>()),"placedMinions":null}),
        generators.is_array(),
        vec![
            "Crafted generators are unlock history, not the currently placed minion setup.".into(),
        ],
    );
    let armor = inventory(m, &["inv_armor"]);
    let equipment = inventory(m, &["equipment_contents"]);
    let carried = inventory(m, &["inv_contents"]);
    let accessories = inventory(m, &["bag_contents.talisman_bag", "talisman_bag"]);
    let ender = inventory(m, &["ender_chest_contents"]);
    let vault = inventory(m, &["personal_vault_contents"]);
    let invalid_loadout = has(m, "loadout") && !m["loadout"].is_object();
    let modern_armor = inv::loadouts(
        if invalid_loadout {
            &Value::Null
        } else {
            at(m, "loadout.armor")
        },
        invalid_loadout || has(m, "loadout.armor"),
        "armor",
    );
    let modern_equipment = inv::loadouts(
        if invalid_loadout {
            &Value::Null
        } else {
            at(m, "loadout.equipment")
        },
        invalid_loadout || has(m, "loadout.equipment"),
        "equipment",
    );
    let legacy = (!invalid_loadout && !has(m, "loadout.armor"))
        .then(|| inventory(m, &["wardrobe_contents"]));
    let wardrobe = legacy.as_ref().unwrap_or(&modern_armor);
    let backpack = inv::backpacks(
        at(m, "inventory.backpack_contents"),
        has(m, "inventory.backpack_contents"),
    );
    let sacks_raw = m["inventory"]
        .get("sacks_counts")
        .filter(|v| !v.is_null())
        .unwrap_or(&m["sack_counts"]);
    let sacks = if sacks_raw.is_object() {
        numeric_map(sacks_raw, 2048)
    } else {
        Value::Null
    };
    let sacks_present = sacks_raw.is_object();
    let mut storage_coverage = json!({});
    for (key, v) in [
        ("enderChestItems", &ender),
        ("personalVaultItems", &vault),
        ("wardrobeItems", wardrobe),
    ] {
        storage_coverage[key] = json!(if v["items"].is_null() {
            "unavailable"
        } else {
            "available"
        });
    }
    storage_coverage["equipmentWardrobeItems"] = modern_equipment["status"].clone();
    if legacy.is_none() {
        storage_coverage["wardrobeItems"] = modern_armor["status"].clone();
    }
    storage_coverage["sackCounts"] = json!(if sacks_present {
        "available"
    } else {
        "unavailable"
    });
    storage_coverage["backpackItems"] = backpack["status"].clone();
    let huntaxe_items = arr(&inv::describe(&carried["items"], items))
        .into_iter()
        .map(|i| merge(i, json!({"location":"carried_inventory"})))
        .chain(
            arr(&inv::describe(&ender["items"], items))
                .into_iter()
                .map(|i| merge(i, json!({"location":"ender_chest"}))),
        )
        .collect::<Vec<_>>();
    let huntaxe = inv::huntaxe(&json!(huntaxe_items));
    let mut field_coverage = json!({});
    for (key, v) in [
        ("armor", &armor),
        ("equipment", &equipment),
        ("carriedItems", &carried),
        ("enderChestItems", &ender),
        ("personalVaultItems", &vault),
        ("wardrobeItems", wardrobe),
        ("equipmentWardrobeItems", &modern_equipment),
    ] {
        field_coverage[key] = json!({"status":v["status"].as_str().unwrap_or(if v["items"].is_null(){"unavailable"}else{"available"}),"reasonCode":v["reasonCode"],"source":if key=="wardrobeItems"{if legacy.is_some(){"inventory.wardrobe_contents"}else{"loadout.armor"}}else if key=="equipmentWardrobeItems"{"loadout.equipment"}else{"inventory"}});
    }
    field_coverage["backpackItems"] = json!({"status":backpack["status"],"reasonCode":if backpack["status"]=="available"{Value::Null}else if !has(m,"inventory.backpack_contents"){json!("not_exposed")}else{json!("decode_or_invalid_api_value")},"source":"inventory.backpack_contents"});
    field_coverage["huntaxeStoredWeapon"] = json!({"status":huntaxe["status"],"reasonCode":text(&huntaxe["reasonCode"],96),"source":text(&huntaxe["source"],96).unwrap_or("Hypixel profile inventory item lore".into())});
    field_coverage["sackCounts"] = json!({"status":if sacks_present{"available"}else{"unavailable"},"reasonCode":if sacks_present{Value::Null}else{json!("not_exposed_or_invalid")},"source":"inventory.sacks_counts"});
    let mut storage_warnings = vec![];
    for (label, v) in [
        ("Ender Chest", &ender),
        ("Personal vault", &vault),
        ("Wardrobe", wardrobe),
    ] {
        storage_warnings.extend(warnings(v).into_iter().map(|w| format!("{label}: {w}")));
    }
    storage_warnings.extend(warnings(&modern_equipment));
    storage_warnings.extend(warnings(&backpack));
    let mut eq_warnings = vec![];
    for (label, v) in [
        ("Armor", &armor),
        ("Equipment", &equipment),
        ("Carried inventory", &carried),
    ] {
        eq_warnings.extend(warnings(v).into_iter().map(|w| format!("{label}: {w}")));
    }
    eq_warnings.push(if huntaxe["status"]=="unavailable"{"No stored-weapon identity was exposed in the observed Huntaxe item lore; this does not establish an empty slot."}else{"Stored-weapon tooltip reports provide a name and displayed stars only; full absorbed-weapon enchantments and upgrades remain unknown. Check for conflicting reports."}.into());
    if obj(&storage_coverage).values().any(|v| v != "available") {
        eq_warnings.push("Some storage fields are unavailable or partial; inspect fieldCoverage and storageWarnings.".into())
    }
    let mut equipment_data = json!({"huntaxeStoredWeapon":huntaxe,"fieldCoverage":field_coverage,"wardrobeSets":if legacy.is_some(){Value::Null}else{modern_armor["sets"].clone()},"equipmentWardrobeSets":modern_equipment["sets"],"equipmentWardrobeItems":inv::describe(&modern_equipment["items"],items),"wardrobeItemCount":if storage_coverage["wardrobeItems"]=="available"{wardrobe["items"].as_array().map(Vec::len)}else{None},"knownWardrobeItemCount":wardrobe["items"].as_array().map(Vec::len),"sackCounts":sacks,"backpackItems":backpack["items"].as_array().map(|a|a.iter().map(|i|merge(inv::describe(&json!([i]),items)[0].clone(),json!({"containerId":i["containerId"]}))).collect::<Vec<_>>()),"backpackContainers":backpack["containers"],"backpackItemCount":if backpack["status"]=="available"{backpack["items"].as_array().map(Vec::len)}else{None},"knownBackpackItemCount":backpack["items"].as_array().map(Vec::len),"storageCoverage":storage_coverage,"storageWarnings":storage_warnings,"itemCatalogLastUpdated":n(&items["lastUpdated"]),"itemCatalogAvailable":!obj(&items["items"]).is_empty()});
    for (key, v) in [
        ("armor", &armor),
        ("equipment", &equipment),
        ("carriedItems", &carried),
        ("enderChestItems", &ender),
        ("personalVaultItems", &vault),
        ("wardrobeItems", wardrobe),
    ] {
        equipment_data[key] = inv::describe(&v["items"], items);
    }
    let eq_present = [
        &armor,
        &equipment,
        &carried,
        wardrobe,
        &modern_equipment,
        &backpack,
    ]
    .iter()
    .any(|v| !v["items"].is_null());
    let equipment_section = section(equipment_data, eq_present, eq_warnings);
    let storage = &m["accessory_bag_storage"];
    let bag = inv::describe(&accessories["items"], items);
    let power = inv::magical_power(&bag, m);
    let diagnostics = inv::accessory_diagnostics(&bag, &inv::describe(&carried["items"], items));
    let mut acc_warnings = warnings(&accessories);
    acc_warnings.push("Highest recorded magical power is historical and may differ from the current snapshot calculation. The calculation applies outside Dungeons and uses pinned community rules.".into());
    acc_warnings.extend(
        arr(&power["reasons"])
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned)),
    );
    acc_warnings.push("Carried accessory lists include only items identified by the catalog or a structured footer. Stored or carried items do not add bag magical power.".into());
    let accessory = section(
        merge(
            merge(
                json!({"bagItems":bag,"selectedPower":text(&storage["selected_power"],96),"unlockedPowers":storage["unlocked_powers"].as_array().map(|a|a.iter().take(256).filter_map(|v|text(v,96)).filter(|s|!s.is_empty()).collect::<Vec<_>>()),"tuning":if has(storage,"tuning.slot_0"){numeric_map(at(storage,"tuning.slot_0"),32)}else{Value::Null},"highestRecordedMagicalPower":n(&storage["highest_magical_power"]),"itemCatalogLastUpdated":n(&items["lastUpdated"]),"itemCatalogAvailable":!obj(&items["items"]).is_empty(),"unclassifiedCarriedItems":inv::describe(&carried["items"],items).as_array().map(|a|a.iter().filter(|i|i["category"].is_null()).count())}),
                diagnostics,
            ),
            power,
        ),
        !accessories["items"].is_null() || !obj(storage).is_empty(),
        acc_warnings,
    );
    let mut garden = p::garden(&Value::Null);
    garden["data"]["greenhouse"] = p::greenhouse(m);
    let mut known = vec![
        ("inventory.inv_contents".into(), carried["items"].clone()),
        ("inventory.inv_armor".into(), armor["items"].clone()),
        (
            "inventory.equipment_contents".into(),
            equipment["items"].clone(),
        ),
        (
            "inventory.ender_chest_contents".into(),
            ender["items"].clone(),
        ),
        (
            "inventory.personal_vault_contents".into(),
            vault["items"].clone(),
        ),
        (
            "inventory.bag_contents.talisman_bag".into(),
            accessories["items"].clone(),
        ),
    ];
    for c in arr(&backpack["containers"]) {
        let id = c["containerId"].as_str().unwrap_or("");
        known.push((
            format!("inventory.backpack_contents.{id}"),
            json!(
                arr(&backpack["items"])
                    .into_iter()
                    .filter(|i| i["containerId"] == id)
                    .collect::<Vec<_>>()
            ),
        ));
    }
    if legacy.is_some() {
        known.push((
            "inventory.wardrobe_contents".into(),
            wardrobe["items"].clone(),
        ))
    }
    let ledger = inv::extra_storage(m, &known);
    let mut sections = merge(
        merge(
            json!({"stats":stats,"skills":p::skills(m,catalog,metadata),"currencies":currencies,"slayers":slayers,"dungeons":p::dungeons(m,Some(owner)),"pets":pets,"collections":p::collections(m,collections),"minions":minions,"equipment":equipment_section,"accessories":accessory,"garden":garden}),
            systems::game_systems(m, raw),
        ),
        additional_sections(m, raw, metadata),
    );
    sections["bestiary"] = systems::bestiary(m);
    sections["storage"] = json!({"status":"partial","data":ledger,"warnings":["Physical item records and view duplicates are distinct. Unidentified duplicate views and unobserved locations prevent a complete ownership total."]});
    sections["data_contract"] = json!({"status":"partial","data":{},"warnings":[]});
    let registry=SECTIONS.iter().filter(|s|**s!="systems").map(|s|json!({"id":s,"status":sections[*s]["status"],"source":"profile_API_or_separately_labeled_client_observations","fullyImplemented":false,"coverage":sections[*s]["data"]["fieldCoverage"]})).collect::<Vec<_>>();
    sections["systems"]["data"]["registry"] = json!(registry);
    Ok(
        json!({"id":id,"name":text(&raw["cute_name"],32).unwrap_or("Unnamed".into()),"selected":raw["selected"]==true,"gameMode":text(&raw["game_mode"],32),"sections":sections}),
    )
}
