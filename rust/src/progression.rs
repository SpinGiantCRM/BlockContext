use crate::{
    factual::{TREE_RULES, game_evidence},
    types::*,
};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::LazyLock};
pub static RULES: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fixtures/domain/progression.json")).unwrap()
});
pub static GARDEN: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fixtures/domain/garden.json")).unwrap()
});
pub static MISC: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fixtures/domain/misc.json")).unwrap()
});
fn nullable_map(v: &Value, limit: usize) -> Value {
    if v.as_object()
        .is_some_and(|o| o.len() <= limit && o.values().all(|v| nonnegative(v).is_some()))
    {
        v.clone()
    } else {
        Value::Null
    }
}
pub fn garden(raw: &Value) -> Value {
    if raw.is_null() {
        return json!({"status":"unavailable","data":{},"warnings":["Garden data is not available for this profile; this does not prove the Garden is locked."]});
    }
    let resources = nullable_map(&raw["resources_collected"], 256);
    let visits = nullable_map(at(raw, "commission_data.visits"), 256);
    let completed = nullable_map(at(raw, "commission_data.completed"), 256);
    let plots = raw["unlocked_plots_ids"].as_array().map(|a| {
        let mut seen = HashSet::new();
        a.iter()
            .take(64)
            .filter_map(|v| text(v, 96))
            .filter(|s| seen.insert(s.clone()))
            .collect::<Vec<_>>()
    });
    let commissions=raw.get("active_commissions").map(|v|obj(v).into_iter().take(64).map(|(id,v)|json!({"visitor":text(&json!(id),96),"status":text(&v["status"],96),"requirements":v["requirement"].as_array().map(|a|a.iter().take(32).map(|r|json!({"item":text(&r["item"],96),"amount":n(&r["amount"]),"originalItem":text(&r["original_item"],96),"originalAmount":n(&r["original_amount"])})).collect::<Vec<_>>())})).collect::<Vec<_>>());
    let experience = nonnegative(&raw["garden_experience"]);
    let mut warnings = vec![
        "Garden data describes the shared profile. Crop counters are Garden milestone progress, not global collection totals. Composter values are the saved API snapshot, not a simulation of current production.",
    ];
    if experience.is_none() || resources.is_null() || plots.is_none() {
        warnings.push("Some Garden fields are unavailable and must not be interpreted as zero.")
    }
    if has(raw, "greenhouse") {
        warnings.push("Greenhouse raw data is not exposed; unverified new fields are omitted.")
    }
    let milestones = resources.as_object().map(|o| {
        o.iter()
            .map(|(id, amount)| {
                let crop = match id.as_str() {
                    "POTATO_ITEM" => "POTATO",
                    "CARROT_ITEM" => "CARROT",
                    "INK_SACK:3" => "COCOA_BEANS",
                    "NETHER_STALK" => "NETHER_WART",
                    "MUSHROOM_COLLECTION" => "MUSHROOM",
                    "DOUBLE_PLANT" => "SUNFLOWER",
                    _ => id,
                };
                merge(
                    json!({"crop":crop,"resourceId":id,"collected":amount}),
                    if GARDEN["data"]["crop_milestones"][crop].is_array() {
                        threshold(
                            amount,
                            &arr(&GARDEN["data"]["crop_milestones"][crop])
                                .iter()
                                .filter_map(n)
                                .collect::<Vec<_>>(),
                            0,
                        )
                    } else {
                        json!({"level":null,"progress":null,"toNext":null,"maximumLevel":null})
                    },
                )
            })
            .collect::<Vec<_>>()
    });
    let sum = |v: &Value| v.as_object().map(|o| o.values().filter_map(n).sum::<f64>());
    let c = at(raw, "composter_data");
    json!({"status":if experience.is_none()||resources.is_null()||plots.is_none(){"partial"}else{"available"},"data":{"experience":experience,"levelProgress":threshold(&json!(experience),&arr(&GARDEN["data"]["garden_exp"]).iter().filter_map(n).collect::<Vec<_>>(),0),"domainRevision":MISC["commit"],"resourcesCollected":resources,"cropMilestones":milestones,"cropUpgradeLevels":nullable_map(&raw["crop_upgrade_levels"],256),"gardenUpgradeLevels":nullable_map(&raw["garden_upgrades"],256),"unlockedPlots":plots,"unlockedPlotCount":plots.as_ref().map(Vec::len),"barnSkin":text(&raw["selected_barn_skin"],96),"visitors":{"visitsByNpc":visits,"completedByNpc":completed,"totalVisits":sum(&visits),"totalCompleted":n(at(raw,"commission_data.total_completed")).or_else(||sum(&completed)),"uniqueServed":n(at(raw,"commission_data.unique_npcs_served")).or_else(||completed.as_object().map(|o|o.values().filter(|v|n(v).is_some_and(|n|n>0.)).count() as f64))},"activeVisitorRequests":commissions,"composter":{"organicMatter":n(&c["organic_matter"]),"fuelUnits":n(&c["fuel_units"]),"compostUnits":n(&c["compost_units"]),"compostItems":n(&c["compost_items"]),"conversionTicks":n(&c["conversion_ticks"]),"lastSavedAt":n(&c["last_save"]),"upgrades":nullable_map(&c["upgrades"],256)}},"warnings":warnings})
}
pub fn collections(m: &Value, resource: &Value) -> Value {
    let mut defs = serde_json::Map::new();
    for (category, g) in obj(&resource["collections"]) {
        for (id, v) in obj(&g["items"]) {
            defs.insert(id, merge(v, json!({"category":category})));
        }
    }
    let amounts = obj(&m["collection"]);
    let unlocks = at(m, "player_data.unlocked_coll_tiers");
    let mut ids = defs
        .keys()
        .chain(amounts.keys())
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    let rows=ids.into_iter().map(|id|{let def=defs.get(&id).unwrap_or(&Value::Null);let amount=amounts.get(&id).and_then(nonnegative);let mut tiers=arr(&def["tiers"]).iter().map(|v|json!({"tier":n(&v["tier"]),"amountRequired":n(&v["amountRequired"]),"unlocks":v["unlocks"].as_array().map(|a|a.iter().filter_map(|v|text(v,256)).collect::<Vec<_>>())})).filter(|v|!v["tier"].is_null()&&!v["amountRequired"].is_null()).collect::<Vec<_>>();tiers.sort_by(|a,b|n(&a["amountRequired"]).unwrap().total_cmp(&n(&b["amountRequired"]).unwrap()));let reached=amount.and_then(|n|tiers.iter().rev().find(|t|t["amountRequired"].as_f64().unwrap()<=n).and_then(|t|crate::types::n(&t["tier"])).or_else(||(!tiers.is_empty()).then_some(0.)));let next=amount.and_then(|n|tiers.iter().find(|t|t["amountRequired"].as_f64().unwrap()>n));json!({"id":id,"name":text(&def["name"],96),"category":def["category"],"amount":amount,"unit":"items","scope":"member","valueKind":"lifetime_collection","status":if amount.is_none(){"unknown"}else if tiers.is_empty(){"partial"}else{"known"},"reasonCode":if amount.is_none(){json!("member_amount_not_exposed")}else if tiers.is_empty(){json!("official_collection_definition_unavailable")}else{Value::Null},"tierByMemberAmount":reached,"reportedUnlockedTiers":unlocks.as_array().map(|a|a.iter().filter(|v|v.as_str().is_some_and(|s|s.starts_with(&format!("{id}_")))).collect::<Vec<_>>()),"coOpTier":null,"coOpReasonCode":"other_members_not_collected_or_combined","nextTier":next,"remainingToNextTier":amount.zip(next).map(|(n,next)|(next["amountRequired"].as_f64().unwrap()-n).max(0.)),"maximumTier":n(&def["maxTiers"]),"tiers":tiers,"physicalResourcesAvailable":null})}).collect::<Vec<_>>();
    json!({"status":if has(m,"collection"){"partial"}else{"unavailable"},"data":{"collected":Value::Object(amounts.iter().filter(|(_,v)|n(v).is_some()).map(|(k,v)|(k.clone(),v.clone())).collect()),"progress":rows,"unlockedRecipes":unlocks,"coverage":{"returned":rows.len(),"officialDefinitions":defs.len(),"reportedAmounts":amounts.len(),"allMemberAmountsKnown":rows.iter().all(|r|!r["amount"].is_null()),"coOpCollectionsKnown":false},"resourceLastUpdated":resource["lastUpdated"]},"warnings":["A missing member collection is unknown, not zero. Official tier thresholds applied to member totals do not establish shared co-op recipe eligibility. Lifetime amounts are distinct from spendable materials."]})
}
pub fn buffs(m: &Value) -> Value {
    fn coverage(status: &str, reason: Value, source: &str) -> Value {
        json!({"status":status,"reasonCode":reason,"source":source})
    }
    fn effects(v: &Value, present: bool, source: &str) -> Value {
        let Some(a) = v.as_array() else {
            return json!({"items":null,"count":null,"knownCount":null,"coverage":coverage("unavailable",json!(if present{"invalid_api_value"}else{"not_exposed"}),source)});
        };
        let (mut invalid, mut incomplete) = (0, 0);
        let mut items = vec![];
        for p in a.iter().take(256) {
            if !p["effect"]
                .as_str()
                .is_some_and(|s| re("^[a-zA-Z0-9_:-]{1,64}$", s))
            {
                invalid += 1;
                continue;
            }
            let level = nonnegative(&p["level"])
                .filter(|n| n.fract() == 0. && *n > 0. && *n <= 9007199254740991.);
            let infinite = p["infinite"].as_bool();
            let duration = if infinite == Some(true) {
                None
            } else {
                nonnegative(&p["duration_ticks"])
                    .filter(|n| n.fract() == 0. && *n <= 9007199254740991.)
            };
            let complete = level.is_some()
                && (duration.is_some() || infinite == Some(true))
                && infinite.is_some();
            if !complete {
                incomplete += 1
            }
            let reason = |key: &str| {
                json!(if has(p, key) {
                    "invalid_api_value"
                } else {
                    "not_exposed"
                })
            };
            items.push(json!({"effect":p["effect"],"level":level,"durationTicks":duration,"infinite":infinite,"status":if complete{"available"}else{"partial"},"fieldCoverage":{"level":coverage(if level.is_some(){"available"}else{"unavailable"},if level.is_some(){Value::Null}else{reason("level")},&format!("{source}.level")),"durationTicks":coverage(if duration.is_some(){"available"}else{"unavailable"},if infinite==Some(true){json!("infinite_effect")}else if duration.is_some(){Value::Null}else{reason("duration_ticks")},&format!("{source}.duration_ticks")),"infinite":coverage(if infinite.is_some(){"available"}else{"unavailable"},if infinite.is_some(){Value::Null}else{reason("infinite")},&format!("{source}.infinite"))}}));
        }
        let truncated = a.len() > 256;
        let partial = truncated || invalid > 0 || incomplete > 0;
        json!({"items":items,"count":if partial{None}else{Some(items.len())},"knownCount":items.len(),"coverage":merge(coverage(if partial{"partial"}else{"available"},if truncated{json!("safety_limit")}else if invalid>0{json!("invalid_effect_entries")}else if incomplete>0{json!("missing_or_invalid_effect_fields")}else{Value::Null},source),json!({"invalidEntries":invalid,"incompleteEntries":incomplete,"truncated":truncated}))})
    }
    let cookie = at(m, "profile.cookie_buff_active").as_bool();
    let active = effects(
        at(m, "player_data.active_effects"),
        has(m, "player_data.active_effects"),
        "member.player_data.active_effects",
    );
    let paused = effects(
        at(m, "player_data.paused_effects"),
        has(m, "player_data.paused_effects"),
        "member.player_data.paused_effects",
    );
    let incomplete = cookie.is_none()
        || active["coverage"]["status"] != "available"
        || paused["coverage"]["status"] != "available";
    let mut warnings = vec![
        "Cookie and potion effects are API-reported snapshots; cache age does not prove their current in-game state. Dedicated God Potion status and cookie/God Potion timers remain unknown; effect lists do not establish a God Potion or its expiry.",
    ];
    if incomplete {
        warnings.push("Some cookie or potion-effect fields are missing, malformed or exceed safety limits; unknown does not mean inactive or an empty list.")
    }
    json!({"anyAvailable":cookie.is_some()||!active["items"].is_null()||!paused["items"].is_null(),"incomplete":incomplete,"data":{"boosterCookie":{"active":cookie,"remainingSeconds":null},"godPotion":{"active":null,"remainingSeconds":null},"activePotionEffects":active["items"],"pausedPotionEffects":paused["items"],"activePotionEffectCount":active["count"],"pausedPotionEffectCount":paused["count"],"knownActivePotionEffectCount":active["knownCount"],"knownPausedPotionEffectCount":paused["knownCount"],"potionTiming":"Saved API tick values, not a live countdown. No wall-clock expiry is inferred."},"fieldCoverage":{"boosterCookieActive":coverage(if cookie.is_some(){"available"}else{"unavailable"},if cookie.is_some(){Value::Null}else{json!(if has(m,"profile.cookie_buff_active"){"invalid_api_value"}else{"not_exposed"})},"member.profile.cookie_buff_active"),"boosterCookieRemainingSeconds":coverage("unavailable",json!("not_exposed_as_verified_timer"),"Hypixel profile API"),"godPotionActive":coverage("unavailable",json!("not_exposed_as_verified_status"),"Hypixel profile API"),"godPotionRemainingSeconds":coverage("unavailable",json!("not_exposed_as_verified_timer"),"Hypixel profile API"),"activePotionEffects":active["coverage"],"pausedPotionEffects":paused["coverage"]},"warnings":warnings})
}
pub fn dungeons(m: &Value, owner: Option<&str>) -> Value {
    let raw = at(m, "dungeons.dungeon_types");
    let (mut types, mut classes, mut coverage) = (json!({}), json!({}), json!({}));
    for (kind, v) in obj(raw).into_iter().take(16) {
        if !re("^[a-z_]{1,32}$", &kind) {
            continue;
        }
        let xp = n(&v["experience"]);
        let calc = if kind == "catacombs" {
            dungeon_progress(&v["experience"])
        } else {
            Value::Null
        };
        let completions = &v["tier_completions"];
        let valid = completions.as_object().is_some_and(|o| {
            o.len() <= 32
                && o.iter().all(|(k, v)| {
                    re("^(?:[0-7]|total)$", k) && nonnegative(v).is_some_and(|n| n.fract() == 0.)
                })
        });
        let highest =
            n(&v["highest_tier_completed"]).filter(|n| n.fract() == 0. && (0. ..=7.).contains(n));
        let next = if calc["status"] == "available" {
            arr(&RULES["normalFloorLevelRequirements"])
                .iter()
                .position(|v| {
                    n(v).is_some_and(|n| n > calc["effectiveLevel"].as_f64().unwrap_or(0.))
                })
        } else {
            None
        };
        let records = Value::Object(
            [
                "best_score",
                "fastest_time",
                "fastest_time_s",
                "fastest_time_s_plus",
                "times_played",
                "milestone_completions",
                "mobs_killed",
                "most_damage_berserk",
                "most_healing",
                "most_mobs_killed",
                "watcher_kills",
            ]
            .iter()
            .filter_map(|k| v.get(*k).map(|v| (k.to_string(), v.clone())))
            .collect(),
        );
        let runs=obj(&v["best_runs"]).into_iter().filter(|(floor,_)|re("^[0-7]$",floor)).flat_map(|(floor,runs)|arr(&runs).into_iter().take(32).map(move |r|merge(game_evidence(&r)["value"].clone(),json!({"floor":floor.parse::<usize>().unwrap_or(0),"partySize":n(&r["reportedPartySize"]).or_else(||r["teammates"].as_array().map(|a|a.len() as f64))})))).collect::<Vec<_>>();
        types[&kind] = json!({"experience":xp,"highestTierCompleted":highest,"completions":if valid{Value::Object(obj(completions).into_iter().filter(|(k,v)|re("^[0-7]$",k)&&nonnegative(v).is_some()).collect())}else{Value::Null},"totalCompletions":if valid{n(&completions["total"])}else{None},"records":game_evidence(&records),"bestRuns":runs,"progression":calc,"effectiveLevel":calc["effectiveLevel"],"nextUnlock":next.map(|i|json!({"kind":"normal_floor_level_gate","floor":i+1,"requiredCatacombsLevel":RULES["normalFloorLevelRequirements"][i],"levelsRemaining":RULES["normalFloorLevelRequirements"][i].as_f64().unwrap_or(0.)-calc["effectiveLevel"].as_f64().unwrap_or(0.),"otherPrerequisitesAssessed":false,"referenceSource":RULES["floorRequirementSource"],"referenceDate":"2021-03-30","referenceReview":RULES["floorRequirementReview"]}))});
        coverage[&kind] = json!({"experience":{"status":if xp.is_some_and(|n|n>=0.){"available"}else{"unavailable"},"reasonCode":if xp.is_some_and(|n|n>=0.){Value::Null}else{json!("missing_or_invalid_experience")}},"highestTierCompleted":{"status":if highest.is_some(){"available"}else{"unavailable"},"reasonCode":if highest.is_some(){Value::Null}else{json!("not_exposed_or_invalid")}},"completions":{"status":if valid{"available"}else{"unavailable"},"reasonCode":if valid{Value::Null}else{json!("not_exposed_or_invalid")}},"level":{"status":calc["status"].as_str().unwrap_or("unavailable"),"reasonCode":if calc.is_null(){json!("unsupported_dungeon_type")}else{calc["reasonCode"].clone()}}});
    }
    for (kind, v) in obj(at(m, "dungeons.player_classes")).into_iter().take(16) {
        if re("^[a-z_]{1,32}$", &kind) {
            classes[&kind] = json!({"experience":n(&v["experience"]),"progression":dungeon_progress(&v["experience"])});
        }
    }
    fn hash(v: &Value) -> Value {
        use sha2::{Digest, Sha256};
        v.as_str()
            .filter(|s| re("(?i)^[a-f0-9-]{32,36}$", s))
            .map(|s| {
                json!(format!(
                    "{:x}",
                    Sha256::digest(s.replace('-', "").to_lowercase().as_bytes())
                ))
            })
            .unwrap_or(Value::Null)
    }
    let treasure = at(m, "dungeons.treasures");
    let runs=arr(&treasure["runs"]).iter().take(128).map(|r|{let party=arr(&r["participants"]);let own=r.get("ownParticipant").filter(|v|!v.is_null()).or_else(||party.iter().find(|p|p["player_uuid"].as_str().or(p["uuid"].as_str()).and_then(uuid).as_deref()==owner));let mut out=json!({"instanceId":r.get("run_instance_id").cloned().unwrap_or_else(||hash(&r["run_id"])),"completedAt":n(&r["completion_ts"]),"floor":n(&r["dungeon_tier"]),"partySize":n(&r["reportedPartySize"]).or_else(||r["participants"].as_array().map(|a|a.len() as f64)),"ownParticipant":own.map(|v|game_evidence(v)["value"].clone()),"performanceComplete":false});for (src,dest) in [("type","type"),("dungeon_type","dungeonType")]{if let Some(v)=r.get(src){out[dest]=v.clone()}}out}).collect::<Vec<_>>();
    let chests=arr(&treasure["chests"]).iter().take(128).map(|c|{let mut out=json!({"runInstanceId":c.get("run_instance_id").cloned().unwrap_or_else(||hash(&c["run_id"])),"chestInstanceId":c.get("chest_instance_id").cloned().unwrap_or_else(||hash(&c["chest_id"])),"quality":n(&c["quality"]),"paid":c["paid"].as_bool(),"rerolls":n(&c["rerolls"]),"rewards":game_evidence(&c["rewards"]),"coinsPaid":null,"profitLoss":null,"reasonCode":"actual_chest_cost_and_sale_proceeds_not_exposed"});for (src,dest) in [("type","type"),("treasure_type","treasureType")]{if let Some(v)=c.get(src){out[dest]=v.clone()}}out}).collect::<Vec<_>>();
    let incomplete = obj(&types).is_empty()
        || obj(&types).values().any(|v| {
            v["progression"]["status"] != "available"
                || v["completions"].is_null()
                || v["highestTierCompleted"].is_null()
        })
        || !has(m, "dungeons.player_classes")
        || obj(&classes)
            .values()
            .any(|v| v["progression"]["status"] != "available");
    let mut warnings = vec![
        "Levels use pinned community XP tables and stop at level 50; overflow XP is separate. Next unlock is the next normal-floor level gate from the cited patch, not permission to enter: Combat level, prior floor completions and any later rule changes must also be checked. Current floor requirements remain unverified; referenceReview records the latest source review.",
    ];
    if incomplete {
        warnings
            .push("Some dungeon XP, class data or floor completions are missing or unsupported.")
    }
    json!({"status":if !raw.is_object(){"unavailable"}else if incomplete{"partial"}else{"available"},"data":{"types":types,"classes":classes,"fieldCoverage":coverage,"recentRuns":runs,"treasureChests":chests,"runCoverage":{"expectedRuns":treasure["runs"].as_array().map(Vec::len),"returnedRuns":runs.len(),"completeHistory":false,"source":"API_reported_recent_runs_only"},"lastDungeonRun":at(m,"dungeons.last_dungeon_run").as_str(),"secrets":n(at(m,"dungeons.secrets")),"dailyRunEvidence":game_evidence(at(m,"dungeons.daily_runs")),"journalEvidence":game_evidence(at(m,"dungeons.dungeon_journal")),"activeDungeonLoadout":null,"activeDungeonLoadoutReasonCode":"open_world_equipment_does_not_prove_dungeon_setup","levelCap":RULES["catacombsCap"],"floorRequirementReview":RULES["floorRequirementReview"]},"warnings":warnings})
}
pub fn progress(xp: f64, steps: &[f64], start: usize) -> Value {
    let mut level = start;
    let mut base = 0.;
    let mut next = None;
    for step in steps {
        if xp >= base + step {
            base += step;
            level += 1;
        } else {
            next = Some(base + step);
            break;
        }
    }
    let maximum: f64 = steps.iter().sum();
    let cap = start + steps.len();
    json!({"level":level,"levelWithProgress":level as f64+next.map(|n|(xp-base)/(n-base)).unwrap_or(0.),"maxLevel":cap,"atCap":level==cap,"experienceIntoCurrentLevel":if next.is_none(){0.}else{xp-base},"experienceToNextLevel":next.map(|n|n-xp),"progressToNextLevel":next.map(|n|(xp-base)/(n-base)),"maximumExperience":maximum,"experienceToMaxLevel":(maximum-xp).max(0.),"progressToMaxLevel":(xp/maximum).min(1.),"overflowExperience":(xp-maximum).max(0.)})
}
pub fn pet_progress(kind: &Value, tier: &Value, xp: &Value) -> Value {
    let unavailable = |reason: &str| json!({"status":"unavailable","reasonCode":reason,"level":null,"maxLevel":null,"experienceToNextLevel":null,"experienceToMaxLevel":null,"progressToMaxLevel":null,"sourceRevision":RULES["sourceCommit"]});
    let Some(xp) = nonnegative(xp) else {
        return unavailable("missing_or_invalid_experience");
    };
    let Some(kind) = kind
        .as_str()
        .filter(|k| arr(&RULES["knownPetTypes"]).contains(&json!(k)))
    else {
        return unavailable("unsupported_pet_type");
    };
    let Some(tier) = tier.as_str() else {
        return unavailable("unsupported_pet_rarity");
    };
    let custom = &RULES["customPetLeveling"][kind];
    let offsets = if custom["rarity_offset"].is_object() {
        &custom["rarity_offset"]
    } else {
        &RULES["petRarityOffsets"]
    };
    let Some(offset) = offsets[tier].unsigned() else {
        return unavailable("unsupported_pet_rarity");
    };
    let mut table = arr(&RULES["petXpSteps"]);
    match custom["type"].unsigned() {
        Some(1) => table.extend(arr(&custom["pet_levels"])),
        Some(2) => table = arr(&custom["pet_levels"]),
        _ => {}
    }
    let cap = custom["max_level"].unsigned().unwrap_or(100) as usize;
    let steps = table
        .into_iter()
        .skip(offset as usize)
        .take(cap - 1)
        .map(|v| n(&v).unwrap_or(-1.))
        .collect::<Vec<_>>();
    if steps.len() != cap - 1 || steps.iter().any(|n| *n <= 0.) {
        return unavailable("unsupported_experience_curve");
    }
    let p = progress(xp, &steps, 1);
    merge(
        p.clone(),
        json!({"status":"available","reasonCode":null,"dragonStage":if cap==200{json!(if p["level"].unsigned().unwrap_or(0)<100{"egg"}else{"hatched"})}else{Value::Null},"sourceRevision":RULES["sourceCommit"]}),
    )
}
pub fn dungeon_progress(xp: &Value) -> Value {
    let Some(xp) = nonnegative(xp) else {
        return json!({"status":"unavailable","reasonCode":"missing_or_invalid_experience","effectiveLevel":null,"experienceToNextLevel":null,"sourceRevision":RULES["sourceCommit"]});
    };
    let p = progress(
        xp,
        &arr(&RULES["catacombsXpSteps"])
            .iter()
            .filter_map(n)
            .collect::<Vec<_>>(),
        0,
    );
    merge(
        p.clone(),
        json!({"status":"available","reasonCode":null,"effectiveLevel":p["level"],"effectiveLevelWithProgress":p["levelWithProgress"],"sourceRevision":RULES["sourceCommit"]}),
    )
}
pub fn pets_section(raw: &Value, present: bool) -> Value {
    let Some(raw) = raw.as_array() else {
        return json!({"status":"unavailable","data":{"pets":null,"fieldCoverage":{"pets":{"status":"unavailable","reasonCode":if present{"invalid_api_value"}else{"not_exposed"}}}},"warnings":["Pet list is not exposed as a valid API array; this does not mean no pets."]});
    };
    let pets=raw.iter().take(512).map(|p|{let calc=pet_progress(&p["type"],&p["tier"],&p["exp"]);let safe=|v:&Value,pattern:&str|v.as_str().filter(|s|re(pattern,s)).map(str::to_owned);json!({"type":safe(&p["type"],"^[A-Z0-9_]{1,64}$"),"tier":safe(&p["tier"],"^[A-Z_]{1,16}$"),"experience":n(&p["exp"]),"active":p["active"].as_bool(),"heldItem":safe(&p["heldItem"],"^[A-Z0-9_]{1,96}$"),"level":calc["level"],"maxLevel":calc["maxLevel"],"progression":calc})}).collect::<Vec<_>>();
    let unknown = pets
        .iter()
        .filter(|p| p["progression"]["status"] != "available")
        .count();
    let active = pets.iter().filter(|p| p["active"].is_null()).count();
    let truncated = raw.len() > 512;
    let mut warnings = vec![
        "Pet levels use pinned community XP tables; unknown types and rarities remain unknown.",
    ];
    if unknown > 0 {
        warnings
            .push("Some pets cannot be converted to levels; inspect each progression reasonCode.")
    }
    if truncated {
        warnings.push("Pet list exceeds the 512-entry safety limit.")
    }
    json!({"status":if unknown>0||active>0||truncated{"partial"}else{"available"},"data":{"pets":pets,"fieldCoverage":{"pets":{"status":if truncated{"partial"}else{"available"},"reasonCode":if truncated{json!("safety_limit")}else{Value::Null}},"levels":{"status":if unknown>0{"partial"}else{"available"},"unknownCount":unknown,"reasonCode":if unknown>0{json!("unsupported_or_missing_pet_data")}else{Value::Null}},"activeFlags":{"status":if active>0{"partial"}else{"available"},"unknownCount":active}}},"warnings":warnings})
}
pub fn skills(m: &Value, catalog: &Value, player: &Value) -> Value {
    let mut values = serde_json::Map::new();
    for (k, v) in obj(m) {
        if let Some(k) = k.strip_prefix("experience_skill_") {
            values.insert(format!("SKILL_{}", k.to_uppercase()), v);
        }
    }
    values.extend(obj(at(m, "player_data.experience")));
    let mut skills = serde_json::Map::new();
    for (key, v) in values.into_iter().take(64) {
        if !re("^SKILL_[A-Z_]{1,32}$", &key) {
            continue;
        }
        let Some(xp) = nonnegative(&v) else { continue };
        let kind = key[6..].to_lowercase();
        let def = &catalog["skills"][&key[6..]];
        let mut levels = arr(&def["levels"]);
        levels.sort_by(|a, b| {
            n(&a["level"])
                .unwrap_or(0.)
                .total_cmp(&n(&b["level"]).unwrap_or(0.))
        });
        let valid = !levels.is_empty()
            && levels.len() <= 256
            && levels.iter().enumerate().all(|(i, l)| {
                n(&l["level"]) == Some((i + 1) as f64)
                    && n(&l["totalExpRequired"]).is_some_and(|n| {
                        n > if i == 0 {
                            0.
                        } else {
                            levels[i - 1]["totalExpRequired"].as_f64().unwrap_or(0.)
                        }
                    })
            });
        let (mut level, mut base, mut next) = (
            if valid { Some(0.) } else { None },
            if valid { Some(0.) } else { None },
            None,
        );
        if valid {
            for l in &levels {
                let required = n(&l["totalExpRequired"]).unwrap();
                if xp >= required {
                    level = n(&l["level"]);
                    base = Some(required);
                } else {
                    next = Some(required);
                    break;
                }
            }
        }
        let maximum = n(&def["maxLevel"])
            .filter(|max| valid && Some(*max) == levels.last().and_then(|l| n(&l["level"])));
        let progress = base.zip(next).map(|(b, n)| (xp - b) / (n - b));
        let (cap, minimum, source) = skill_cap(&kind, m, player, maximum);
        let effective = level.and_then(|l| {
            cap.map(|cap| l.min(cap))
                .or_else(|| minimum.filter(|min| l <= *min).map(|_| l))
        });
        let uncertain = cap.is_none() && level.zip(minimum).is_some_and(|(l, min)| l >= min);
        let capped = effective.zip(cap).is_some_and(|(l, c)| l >= c);
        let unlocks = next
            .and_then(|n| {
                levels
                    .iter()
                    .find(|l| l["totalExpRequired"].as_f64() == Some(n))
            })
            .and_then(|l| l["unlocks"].as_array())
            .map(|a| {
                a.iter()
                    .take(24)
                    .filter_map(|v| text(v, 256))
                    .collect::<Vec<_>>()
            });
        skills.insert(kind,json!({"experience":xp,"xpLevel":level,"xpLevelWithProgress":level.map(|l|l+progress.unwrap_or(0.)),"experienceToNextLevel":next.map(|n|n-xp),"experienceIntoCurrentLevel":base.map(|b|xp-b),"progressToNextLevel":progress,"catalogMaximumLevel":maximum,"catalogMaximumReached":maximum.map(|m|level==Some(m)),"playerLevelCap":cap,"playerLevelCapMinimum":minimum,"playerLevelCapMaximum":maximum,"capSource":source,"effectiveLevel":effective,"effectiveLevelWithProgress":effective.filter(|_|!uncertain).map(|l|if capped{l}else{l+progress.unwrap_or(0.)}),"atPlayerCap":if cap.is_some()&&effective.is_some(){Some(capped)}else{None},"effectiveExperienceToNextLevel":if effective.is_none()||capped||uncertain{None}else{next.map(|n|n-xp)},"nextLevelUnlocks":unlocks}));
    }
    let mut warnings = vec![
        "XP-derived levels and next-level unlocks are reference data. Use effectiveLevel and playerLevelCap for capped progression; unknown cap gates remain explicit. Social XP is for this member, not a co-op aggregate.",
    ];
    if skills.values().any(|v| v["xpLevel"].is_null()) {
        warnings.push("Some skill tables are unavailable or invalid; their XP is provided without level conversion.")
    }
    json!({"status":if skills.is_empty(){"unavailable"}else{"partial"},"data":{"skills":skills,"catalogLastUpdated":n(&catalog["lastUpdated"])},"warnings":warnings})
}
fn skill_cap(
    kind: &str,
    m: &Value,
    p: &Value,
    max: Option<f64>,
) -> (Option<f64>, Option<f64>, &'static str) {
    match kind {
        "farming" => {
            let perks = at(m, "jacobs_contest.perks");
            let bonus = &perks["farming_level_cap"];
            if perks.is_object() && !has(perks, "farming_level_cap") {
                let min = 50_f64.min(max.unwrap_or(50.));
                (
                    Some(min),
                    Some(min),
                    "Jacobs Contest perks: no cap upgrade recorded.",
                )
            } else if n(bonus).is_some_and(|n| n.fract() == 0. && (0. ..=10.).contains(&n)) {
                (
                    Some((50. + n(bonus).unwrap()).min(max.unwrap_or(60.))),
                    Some(50.),
                    "Jacobs Contest farming_level_cap upgrades.",
                )
            } else {
                (
                    None,
                    Some(50.),
                    "Farming cap upgrades are unavailable or invalid.",
                )
            }
        }
        "taming" => {
            let pets = &m["pets_data"];
            let care = &pets["pet_care"];
            let sacrificed = &care["pet_types_sacrificed"];
            if pets.is_object()
                && (!has(pets, "pet_care") || care.is_object())
                && !has(care, "pet_types_sacrificed")
            {
                let min = 50_f64.min(max.unwrap_or(50.));
                (
                    Some(min),
                    Some(min),
                    "No George sacrifices recorded in pets_data.",
                )
            } else if let Some(a) = sacrificed.as_array().filter(|a| {
                a.len() <= 64
                    && a.iter()
                        .all(|v| v.as_str().is_some_and(|s| re("^[A-Z_]{1,64}$", s)))
            }) {
                let count = a
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<HashSet<_>>()
                    .len();
                if count <= 10 {
                    (
                        Some((50. + count as f64).min(max.unwrap_or(60.))),
                        Some(50.),
                        "Unique pet types sacrificed to George.",
                    )
                } else {
                    (
                        None,
                        Some(50.),
                        "George sacrifice data is unavailable or invalid.",
                    )
                }
            } else {
                (
                    None,
                    Some(50.),
                    "George sacrifice data is unavailable or invalid.",
                )
            }
        }
        "runecrafting" => {
            let ranks = [
                "rank",
                "packageRank",
                "newPackageRank",
                "monthlyPackageRank",
            ]
            .iter()
            .filter_map(|k| p[*k].as_str())
            .collect::<Vec<_>>();
            let paid = [
                "VIP",
                "VIP_PLUS",
                "MVP",
                "MVP_PLUS",
                "SUPERSTAR",
                "YOUTUBER",
                "ADMIN",
                "MODERATOR",
                "GAME_MASTER",
                "HELPER",
            ];
            let any = ranks.iter().any(|r| paid.contains(r));
            let known = !ranks.is_empty()
                && ranks
                    .iter()
                    .all(|r| paid.contains(r) || ["NONE", "NORMAL"].contains(r));
            (
                if any {
                    Some(25_f64.min(max.unwrap_or(25.)))
                } else if known {
                    Some(3.)
                } else {
                    None
                },
                Some(3.),
                if any || known {
                    "Hypixel player rank metadata."
                } else {
                    "Rank metadata unavailable or unrecognized."
                },
            )
        }
        "foraging" => (
            None,
            Some(50.),
            "Galatea/Torrhus unlock gates are not fully exposed by the profile API; catalog maximum is not the player cap.",
        ),
        _ => (max, max, "Current Hypixel skill resource table."),
    }
}
pub fn threshold(amount: &Value, requirements: &[f64], first: usize) -> Value {
    let unknown = json!({"level":null,"progress":null,"toNext":null,"maximumLevel":null});
    let Some(xp) = nonnegative(amount) else {
        return unknown;
    };
    if requirements.iter().any(|n| !n.is_finite() || *n < 0.) {
        return unknown;
    }
    let (mut level, mut total) = (first, 0.);
    for needed in requirements {
        if xp < total + needed {
            return json!({"level":level,"progress":(xp-total)/needed,"toNext":total+needed-xp,"maximumLevel":first+requirements.len()});
        }
        total += needed;
        level += 1;
    }
    json!({"level":level,"progress":null,"toNext":null,"maximumLevel":level})
}
pub fn slayers(m: &Value) -> Value {
    Value::Object(obj(at(m,"slayer.slayer_bosses")).into_iter().map(|(id,v)|{let xp=nonnegative(&v["xp"]);let curve=TREE_RULES["slayerThresholds"][&id].as_array();let level=xp.zip(curve).map(|(xp,c)|c.iter().filter(|v|n(v).is_some_and(|n|xp>=n)).count());let next=xp.zip(curve).and_then(|(xp,c)|c.iter().filter_map(n).find(|n|*n>xp));let kills=Value::Object(obj(&v).into_iter().filter(|(k,v)|re(r"^boss_kills_tier_\d+$",k)&&n(v).is_some()).collect());(id,json!({"experience":xp,"level":level,"cap":curve.map(Vec::len),"experienceToNextLevel":xp.zip(next).map(|(xp,n)|n-xp),"nextLevelThreshold":next,"killsByTier":kills,"reportedClaimedLevels":v["claimed_levels"],"unlockedBossTiers":null,"rewardClaimsVerified":false,"reasonCode":if curve.is_some(){"boss_unlock_and_reward_rules_not_verified"}else{"family_level_curve_not_verified"},"rules":{"source":"https://github.com/SkyCryptWebsite/SkyCrypt-Backend","revision":TREE_RULES["counterRulesRevision"],"liveGameRulesVerified":false}}))}).collect())
}
pub fn greenhouse(m: &Value) -> Value {
    fn mutations(v: &Value) -> Option<Vec<String>> {
        let a = v.as_array()?;
        if a.len() > 256
            || a.iter()
                .any(|v| !v.as_str().is_some_and(|s| re("^[a-z0-9_]{1,64}$", s)))
        {
            return None;
        }
        let mut result = a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        result.sort();
        result.dedup();
        Some(result)
    }
    let d = mutations(at(m, "garden_player_data.discovered_greenhouse_crops"));
    let a = mutations(at(m, "garden_player_data.analyzed_greenhouse_crops"));
    let mut warnings = vec![
        "Greenhouse mutations belong to the requested player, not the shared Garden. Missing lists are unknown, not proof that the Greenhouse is locked or that no mutations exist. Live layout, water, growth and harvest timing are not exposed by this tool.",
    ];
    if d.is_none() || a.is_none() {
        warnings.push("One or both mutation lists are unavailable or invalid; complete progression cannot be established.")
    }
    let inconsistent = d
        .as_ref()
        .zip(a.as_ref())
        .is_some_and(|(d, a)| a.iter().any(|i| !d.contains(i)));
    if inconsistent {
        warnings.push("Some analyzed mutations are absent from the discovered list; the API lists are inconsistent.")
    }
    json!({"status":if d.is_some()&&a.is_some()&&!inconsistent{"available"}else if d.is_none()&&a.is_none(){"unavailable"}else{"partial"},"data":{"discoveredMutations":d,"analyzedMutations":a,"discoveredCount":d.as_ref().map(Vec::len),"analyzedCount":a.as_ref().map(Vec::len),"pendingAnalysis":d.as_ref().zip(a.as_ref()).map(|(d,a)|d.iter().filter(|id|!a.contains(id)).collect::<Vec<_>>()),"layout":null,"water":null,"growth":null,"harvestTiming":null},"warnings":warnings})
}
