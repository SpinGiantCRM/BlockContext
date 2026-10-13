use crate::{
    factual::{DEFINITIONS, TREE_RULES, evidence, field},
    types::*,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
enum Ast {
    Atom(String),
    List(Vec<Ast>),
}
pub fn rule_number(expression: &Value, vars: &HashMap<String, f64>) -> Value {
    fn parse(tokens: &[String], cursor: &mut usize, budget: &mut i32, depth: usize) -> Option<Ast> {
        *budget -= 1;
        if *budget < 0 || depth > 20 {
            return None;
        }
        let token = tokens.get(*cursor)?;
        *cursor += 1;
        if token == "(" {
            let mut items = vec![];
            while tokens.get(*cursor)? != ")" {
                items.push(parse(tokens, cursor, budget, depth + 1)?);
            }
            *cursor += 1;
            Some(Ast::List(items))
        } else if token == ")" {
            None
        } else {
            Some(Ast::Atom(token.clone()))
        }
    }
    fn calc(
        ast: &Ast,
        vars: &HashMap<String, f64>,
        budget: &mut i32,
        depth: usize,
    ) -> Option<Value> {
        *budget -= 1;
        if *budget < 0 || depth > 20 {
            return None;
        }
        match ast {
            Ast::Atom(t) => {
                if re(r"^-?\d+(?:\.\d+)?$", t) {
                    Some(json!(t.parse::<f64>().ok()?))
                } else if let Some(v) = vars.get(t) {
                    Some(json!(v))
                } else if re("^[A-Z_]+$", t) {
                    Some(json!(t))
                } else {
                    None
                }
            }
            Ast::List(items) => {
                let Ast::Atom(op) = items.first()? else {
                    return None;
                };
                let args = &items[1..];
                if op == "if" {
                    if args.len() != 3 {
                        return None;
                    }
                    let truth = calc(&args[0], vars, budget, depth + 1)?;
                    let truth = truth.as_bool().unwrap_or_else(|| {
                        truth.as_f64().is_some_and(|n| n != 0.)
                            || truth.as_str().is_some_and(|s| !s.is_empty())
                    });
                    return calc(&args[if truth { 1 } else { 2 }], vars, budget, depth + 1);
                }
                let values = args
                    .iter()
                    .map(|a| calc(a, vars, budget, depth + 1))
                    .collect::<Option<Vec<_>>>()?;
                if op == "list.new" {
                    return Some(json!(values));
                }
                if op == "list.at" {
                    let a = values.first()?.as_array()?;
                    let index = n(values.get(1)?)?;
                    if index < 0. || index.fract() != 0. {
                        return None;
                    }
                    return a.get(index as usize).cloned();
                }
                let ns = values.iter().map(n).collect::<Option<Vec<_>>>()?;
                let first = || ns.first().copied();
                let two = || Some((ns.first().copied()?, ns.get(1).copied()?));
                let value = match op.as_str() {
                    "+" => ns.iter().sum(),
                    "*" => ns.iter().product(),
                    "-" => {
                        if ns.len() == 1 {
                            -first()?
                        } else {
                            let (a, b) = two()?;
                            a - b
                        }
                    }
                    "/" => {
                        let (a, b) = two()?;
                        a / b
                    }
                    "pow" => {
                        let (a, b) = two()?;
                        a.powf(b)
                    }
                    "floor" => first()?.floor(),
                    "round" => (first()? + 0.5).floor(),
                    "round-decimals" => {
                        let (a, b) = two()?;
                        let scale = 10_f64.powf(b);
                        (a * scale + 0.5).floor() / scale
                    }
                    "min" => ns.iter().copied().reduce(f64::min)?,
                    "max" => ns.iter().copied().reduce(f64::max)?,
                    "id" => first()?,
                    "lt" => {
                        let (a, b) = two()?;
                        return Some(json!(a < b));
                    }
                    "gt" => {
                        let (a, b) = two()?;
                        return Some(json!(a > b));
                    }
                    "=" => {
                        let (a, b) = two()?;
                        return Some(json!(a == b));
                    }
                    _ => return None,
                };
                value.is_finite().then(|| json!(value))
            }
        }
    }
    if expression.is_number() {
        return expression.clone();
    }
    let Some(s) = expression
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 1024)
    else {
        return Value::Null;
    };
    let r = regex::Regex::new(r"[()]|[^\s()]+").unwrap();
    let tokens = r
        .find_iter(s)
        .map(|m| m.as_str().to_owned())
        .collect::<Vec<_>>();
    let (mut cursor, mut budget) = (0, 512);
    let Some(ast) = parse(&tokens, &mut cursor, &mut budget, 0) else {
        return Value::Null;
    };
    if cursor != tokens.len() {
        return Value::Null;
    }
    let Some(v) = calc(&ast, vars, &mut budget, 0) else {
        return Value::Null;
    };
    if v.is_string() || n(&v).is_some_and(|n| n.abs() <= 1e15) {
        v
    } else {
        Value::Null
    }
}
pub fn tree_nodes(kind: &str, presets: &[(String, Value)], tier: Option<f64>) -> Vec<Value> {
    let refs = &DEFINITIONS["trees"][kind]["perks"];
    let mut nodes = vec![];
    for (preset, raw) in presets {
        let mut ids = obj(refs).keys().cloned().collect::<Vec<_>>();
        for id in obj(raw).keys().filter(|k| !k.starts_with("toggle_")) {
            if !ids.contains(id) {
                ids.push(id.clone())
            }
        }
        for id in ids.into_iter().filter(|k| re("^[a-z0-9_]{1,64}$", k)) {
            let def = &refs[&id];
            let level = nonnegative(&raw[&id]).filter(|n| n.fract() == 0.);
            let max = n(&def["maxLevel"]);
            let vars = HashMap::from([
                ("level".into(), level.unwrap_or(0.).max(1.)),
                ("level0".into(), level.unwrap_or(0.)),
                ("maxLevel".into(), max.unwrap_or(0.)),
                ("hotm".into(), tier.unwrap_or(0.)),
            ]);
            let effects = Value::Object(
                obj(def)
                    .into_iter()
                    .filter(|(k, _)| k.starts_with("stat"))
                    .map(|(k, v)| {
                        (
                            k,
                            if level.is_none() {
                                Value::Null
                            } else {
                                rule_number(&v, &vars)
                            },
                        )
                    })
                    .collect(),
            );
            let cost = if level.zip(max).is_some_and(|(l, m)| l > 0. && l < m) {
                rule_number(&def["cost"], &vars)
            } else {
                Value::Null
            };
            let powder = if def["powder"].as_str().is_some_and(|s| s.starts_with('(')) {
                rule_number(&def["powder"], &vars)
            } else {
                json!(text(&def["powder"], 96))
            };
            nodes.push(json!({"id":id,"preset":preset,"name":text(&def["name"],96),"level":level,"maximumLevel":max,"unlocked":level.map(|l|l>0.),"enabled":raw[format!("toggle_{id}")].as_bool(),"nextLevelCost":n(&cost).map(|n|json!({"amount":n.floor(),"currency":powder,"status":"calculated_from_pinned_reference"})),"effects":if level.is_none(){Value::Null}else if level==Some(0.){json!({})}else{effects},"effectMeaning":level.map(|_|json!({"templates":def["lore"],"effectiveGearModifiersIncluded":false})),"isAbility":def["isAbility"].as_bool(),"prerequisites":null,"layout":{"x":def["x"],"y":def["y"]},"definitionReasonCode":if obj(def).is_empty(){json!("node_definition_unavailable")}else if level.is_none(){json!("node_level_not_exposed")}else if cost.is_null()&&level.zip(max).is_some_and(|(l,m)|l<m){json!("next_cost_not_present_in_reference")}else{Value::Null},"rules":{"source":DEFINITIONS["source"],"revision":DEFINITIONS["revision"],"reviewedAt":DEFINITIONS["reviewedAt"],"liveGameRulesVerified":false}}));
        }
    }
    nodes
}
pub fn tree(m: &Value, kind: &str) -> Value {
    let core_kind = if kind == "mining" {
        "mining_core"
    } else {
        "foraging_core"
    };
    let modern = at(m, &format!("skill_tree.experience.{kind}"));
    let xp = nonnegative(if modern.is_null() {
        &m[core_kind]["experience"]
    } else {
        modern
    });
    let curve = arr(&TREE_RULES["cumulativeExperience"][kind]);
    let (mut level, mut next, mut base) = (xp.map(|_| 0.), None, None);
    if let Some(xp) = xp {
        for (i, v) in curve.iter().enumerate() {
            if xp >= n(v).unwrap_or(f64::INFINITY) {
                level = Some((i + 1) as f64);
                base = n(v)
            } else {
                next = n(v);
                break;
            }
        }
    }
    let modern_nodes = at(m, "skill_tree.nodes");
    let nodes = if modern_nodes.is_null() {
        &m[core_kind]["nodes"]
    } else {
        modern_nodes
    };
    let mut presets = obj(nodes)
        .into_iter()
        .filter(|(k, _)| k == kind || re(&format!("^{kind}_[2-5]$"), k))
        .collect::<Vec<_>>();
    if presets.is_empty() {
        presets.push((kind.into(), json!(obj(nodes))))
    }
    let nodes = tree_nodes(kind, &presets, level);
    let core_rules = &TREE_RULES["treeTokens"][kind];
    let core_id = core_rules["coreId"].as_str().unwrap_or("");
    let core = nonnegative(&m["skill_tree"]["nodes"][kind][core_id]);
    let spent = nonnegative(
        &m["skill_tree"]["tokens_spent"][if kind == "mining" {
            "mountain"
        } else {
            "forest"
        }],
    );
    let total = level
        .zip(core)
        .filter(|(_, c)| *c <= n(&core_rules["maxCoreLevel"]).unwrap_or(0.))
        .map(|(l, c)| {
            arr(&core_rules["tier"])
                .iter()
                .take(l as usize)
                .filter_map(n)
                .sum::<f64>()
                + arr(&core_rules["core"])
                    .iter()
                    .take(c as usize)
                    .filter_map(n)
                    .sum::<f64>()
        });
    let f = |v: Value, source: &str, unit: Option<&str>| field(v, source, unit, "member");
    json!({"experience":f(json!(xp),&if has(m,&format!("skill_tree.experience.{kind}")){format!("member.skill_tree.experience.{kind}")}else{format!("member.{core_kind}.experience")},Some("XP")),"level":f(json!(level),"calculated_from_reported_tree_experience",Some("tier")),"experienceIntoTier":f(json!(xp.zip(base).map(|(x,b)|x-b)),"calculated",Some("XP")),"experienceToNextTier":f(json!(xp.zip(next).map(|(x,n)|n-x)),"calculated",Some("XP")),"cap":curve.len(),"atCap":level.map(|l|l==curve.len()as f64),"nodes":nodes,"nodesCoverage":{"returned":nodes.len(),"presetEntriesMayBeLimited":true,"allLockedNodesKnown":false,"definitionsAvailable":true,"pinnedReferenceMayBeOlderThanGame":true},"selectedPreset":f(m["skill_tree"]["selected_skill_tree_slot"][kind].clone(),&format!("member.skill_tree.selected_skill_tree_slot.{kind}"),None),"selectedAbility":f(m["skill_tree"]["selected_ability"][kind].clone(),&format!("member.skill_tree.selected_ability.{kind}"),None),"tokensSpent":f(json!(spent),"member.skill_tree.tokens_spent",Some("tokens")),"tokensAvailable":f(json!(total.zip(spent).filter(|(t,s)|t>=s).map(|(t,s)|t-s)),"calculated_from_reported_tier_core_spent_and_pinned_rewards",Some("tokens")),"tokensEarned":f(json!(total),"calculated_pinned_tree_rewards",Some("tokens")),"core":{"id":core_id,"level":f(json!(core),&format!("member.skill_tree.nodes.{kind}.{core_id}"),None),"maximumLevel":core_rules["maxCoreLevel"]},"resetEvidence":evidence(at(m,"skill_tree.last_reset"),"member.skill_tree.last_reset"),"freeResetReportedFlag":f(at(m,"skill_tree.refund_ability_free").clone(),"member.skill_tree.refund_ability_free",None),"rules":{"source":TREE_RULES["source"],"revision":TREE_RULES["commit"],"reviewedAt":TREE_RULES["reviewedAt"],"kind":"pinned_community_cumulative_experience","liveGameRulesVerified":false}})
}
fn counter(v: &Value) -> Option<f64> {
    nonnegative(v).filter(|n| n.fract() == 0. && *n <= 9007199254740991.)
}
pub fn whispers(m: &Value) -> Value {
    let selected = counter(at(m, "skill_tree.selected_skill_tree_slot.foraging"));
    let modern = at(m, "foraging_core.whispers");
    let mut currencies = json!({});
    for kind in ["forest", "desert"] {
        let root = &modern[kind];
        let path = format!("member.foraging_core.whispers.{kind}");
        let u = format!("{kind} Whispers");
        let presets=obj(root).into_iter().filter(|(k,_)|re("^[1-5]$",k)).map(|(preset,v)|json!({"preset":preset.parse::<usize>().unwrap_or(0),"spent":field(json!(counter(&v["spent"])),&format!("{path}.{preset}.spent"),Some(&u),"member"),"spentNonRefundable":field(json!(counter(&v["spent_non_refundable"])),&format!("{path}.{preset}.spent_non_refundable"),Some(&u),"member")})).collect::<Vec<_>>();
        let active = presets.iter().find(|p| n(&p["preset"]) == selected);
        currencies[kind] = json!({"reportedTotal":merge(field(json!(counter(&root["total"])),&format!("{path}.total"),Some(&u),"member"),json!({"meaning":"API counter named total; spendable-balance arithmetic is not yet verified."})),"presets":presets,"selectedPreset":field(json!(selected),"member.skill_tree.selected_skill_tree_slot.foraging",Some("preset"),"member"),"spent":active.map(|v|v["spent"].clone()).unwrap_or_else(||merge(field(Value::Null,&format!("{path}.[selected_preset].spent"),Some(&u),"member"),json!({"reasonCode":"selected_preset_not_exposed_or_spending_missing"}))),"available":merge(field(Value::Null,&path,Some(&u),"member"),json!({"reasonCode":"modern_total_and_spend_semantics_not_verified"})),"semanticStatus":"reported_counters_only","reference":{"source":"https://github.com/SkyblockerMod/Skyblocker/blob/32ed634695a54f97cf8ae2dc234bd7bedfbfcc2f/src/main/java/de/hysky/skyblocker/skyblock/profileviewer2/model/ForagingCore.java","liveGameVerified":false}});
    }
    let available = counter(at(m, "foraging_core.forests_whispers"));
    let spent = counter(at(m, "foraging_core.forests_whispers_spent"));
    let av = field(
        json!(available),
        "member.foraging_core.forests_whispers",
        Some("Forest Whispers"),
        "member",
    );
    let sp = field(
        json!(spent),
        "member.foraging_core.forests_whispers_spent",
        Some("Forest Whispers"),
        "member",
    );
    let is_modern = has(m, "foraging_core.whispers");
    json!({"schema":if is_modern{"modern_per_preset"}else{"legacy"},"forest":currencies["forest"],"desert":currencies["desert"],"available":if is_modern{currencies["forest"]["available"].clone()}else{av.clone()},"spent":if is_modern{currencies["forest"]["spent"].clone()}else{sp.clone()},"total":if is_modern{currencies["forest"]["reportedTotal"].clone()}else{field(json!(available.zip(spent).map(|(a,s)|a+s)),"calculated_from_verified_legacy_available_and_spent",Some("Forest Whispers"),"member")},"reconciliation":{"status":"partial","crossSourceEqualityVerified":false,"reasonCode":if is_modern{"modern_counters_need_displayed_balance_verification"}else{"legacy_counter_reference_only"}},"legacyReported":{"available":av,"spent":sp}})
}
pub fn bestiary(m: &Value) -> Value {
    let raw = obj(at(m, "bestiary.kills"));
    let defs = &DEFINITIONS["bestiary"];
    let refs = arr(&defs["mobs"]);
    let families=refs.iter().map(|r|{let counts=arr(&r["ids"]).iter().map(|id|json!({"id":id,"kills":id.as_str().and_then(|id|raw.get(id)).and_then(n)})).collect::<Vec<_>>();let complete=!counts.is_empty()&&counts.iter().all(|c|nonnegative(&c["kills"]).is_some());let known=counts.iter().filter_map(|c|nonnegative(&c["kills"])).sum::<f64>();let kills=complete.then_some(known);let bracket=r["bracket"].as_str().map(str::to_owned).unwrap_or_else(||r["bracket"].to_string());let tiers=arr(&defs["brackets"][&bracket]).iter().filter_map(n).filter(|v|n(&r["cap"]).is_some_and(|cap|*v<=cap)).collect::<Vec<_>>();let level=kills.filter(|_|!tiers.is_empty()).map(|kills|tiers.iter().filter(|n|kills>=**n).count());let next=kills.and_then(|kills|tiers.iter().copied().find(|n|*n>kills));merge(r.clone(),json!({"kills":kills,"knownKillsSubtotal":known,"tier":level,"nextTierThreshold":next,"remainingToNextTier":kills.zip(next).map(|(k,n)|n-k),"counts":counts,"status":if complete{"known"}else if known>0.{"partial"}else{"unknown"},"reasonCode":if complete{Value::Null}else{json!("one_or_more_member_kill_counters_not_exposed")},"rewardClaims":null}))}).collect::<Vec<_>>();
    json!({"status":if has(m,"bestiary"){"partial"}else{"unavailable"},"data":{"families":families,"reportedKills":raw,"coverage":{"referenceFamilies":refs.len(),"returned":families.len(),"fullyKnown":families.iter().filter(|r|r["status"]=="known").count(),"complete":false},"rules":{"source":DEFINITIONS["source"],"revision":DEFINITIONS["revision"],"liveGameRulesVerified":false}},"warnings":["Missing mob counters remain unknown. Family thresholds use pinned community data; reward claims and effective combat bonuses are not inferred."]})
}
pub fn with_displayed_whispers(api: &Value, client: &Value) -> Value {
    let mut reports = vec![];
    for (key, row) in obj(&client["observations"]) {
        if !["foraging", "tabStats"].contains(&key.as_str()) {
            continue;
        }
        for fact in arr(at(&row, "values.facts")) {
            for f in arr(&fact["fields"]) {
                let label = f["label"].as_str().unwrap_or("");
                if !["Forest Whispers", "Desert Whispers"].contains(&label) {
                    continue;
                }
                let Some(value) = counter(&f["value"]) else {
                    continue;
                };
                reports.push(json!({"currency":if label=="Forest Whispers"{"forest"}else{"desert"},"value":value,"unit":label,"status":if row["currentStateVerified"]==true{"known"}else{"stale"},"source":"client_observation","sourceKey":key,"observedAt":row["observedAt"],"currentStateVerified":row["currentStateVerified"]==true}));
            }
        }
    }
    let balance = |kind: &str, previous: &Value| {
        let all = reports
            .iter()
            .filter(|r| r["currency"] == kind)
            .collect::<Vec<_>>();
        let current = all
            .iter()
            .filter(|r| r["currentStateVerified"] == true)
            .collect::<Vec<_>>();
        let values = current
            .iter()
            .filter_map(|r| n(&r["value"]))
            .map(|v| v.to_bits())
            .collect::<HashSet<_>>();
        if values.len() == 1 {
            return merge(
                (*current[0]).clone(),
                json!({"scope":"member","reasonCode":null}),
            );
        }
        let unknown = |reason: &str| {
            merge(
                field(
                    Value::Null,
                    "client.foraging",
                    Some(&format!("{kind} Whispers")),
                    "member",
                ),
                json!({"reasonCode":reason}),
            )
        };
        if values.len() > 1 {
            return unknown("conflicting_current_displayed_balances");
        }
        if previous["status"] == "known" {
            return previous.clone();
        }
        let Some(latest) = all
            .iter()
            .max_by_key(|r| r["observedAt"].as_str().unwrap_or(""))
        else {
            return previous.clone();
        };
        let same = all
            .iter()
            .filter(|r| r["observedAt"] == latest["observedAt"])
            .filter_map(|r| n(&r["value"]))
            .map(f64::to_bits)
            .collect::<HashSet<_>>();
        if same.len() > 1 {
            return unknown("conflicting_saved_displayed_balances");
        }
        merge(
            (**latest).clone(),
            json!({"status":"stale","scope":"member","currentStateVerified":false,"reasonCode":"last_displayed_balance_not_verified_current"}),
        )
    };
    merge(
        api.clone(),
        json!({"available":balance("forest",&api["available"]),"apiAvailable":api["available"],"forest":merge(api["forest"].clone(),json!({"available":balance("forest",&api["forest"]["available"]),"apiAvailable":api["forest"]["available"]})),"desert":merge(api["desert"].clone(),json!({"available":balance("desert",&api["desert"]["available"]),"apiAvailable":api["desert"]["available"]})),"displayedBalances":reports,"reconciliation":merge(api["reconciliation"].clone(),json!({"displayedBalanceObserved":!reports.is_empty(),"currentDisplayedBalanceVerified":reports.iter().any(|r|r["currentStateVerified"]==true),"crossSourceEqualityVerified":false}))}),
    )
}
fn partial(data: Value, present: bool, extra: Vec<String>) -> Value {
    let mut warnings=vec!["Missing or undocumented fields stay unknown. Reported API evidence is not a complete model of this system.".to_owned()];
    warnings.extend(extra);
    json!({"status":if present{"partial"}else{"unavailable"},"data":data,"warnings":warnings})
}
fn activities(m: &Value) -> Vec<Value> {
    let defs = [
        (
            "hotm_bonus_commissions",
            "HotM daily bonus commissions",
            Value::Null,
        ),
        (
            "experimentation",
            "Experimentation Table",
            m["experimentation"].clone(),
        ),
        (
            "foraging_gifts",
            "Foraging gifts",
            m["foraging_core"].clone(),
        ),
        ("forge", "Forge jobs", m["forge"].clone()),
        (
            "dungeons_daily",
            "Dungeon daily runs",
            at(m, "dungeons.daily_runs").clone(),
        ),
        (
            "garden",
            "Garden visitors, pests and composter",
            m["garden_player_data"].clone(),
        ),
        (
            "chocolate_factory",
            "Chocolate Factory",
            at(m, "events.easter").clone(),
        ),
        (
            "crimson_daily",
            "Crimson Isle daily activities",
            m["nether_island_player_data"].clone(),
        ),
        (
            "community_upgrades",
            "Community upgrades",
            m["community_upgrades"].clone(),
        ),
    ];
    defs.iter().map(|(id,name,value)|json!({"activityId":id,"name":name,"status":if value.is_null(){"unknown"}else{"partial"},"eligible":null,"completedToday":null,"remainingCharges":null,"nextReset":null,"reward":null,"certainty":"No completion is inferred from a missing field or an undocumented timestamp.","evidence":evidence(value,&format!("{}.{id}",if *id=="community_upgrades"{"profile"}else{"member"}))})).collect()
}
pub fn game_systems(m: &Value, raw: &Value) -> Value {
    let mining = tree(m, "mining");
    let foraging = tree(m, "foraging");
    let powders=["mithril","gemstone","glacite"].iter().map(|kind|{let f=|key:&str|field(json!(nonnegative(&m["mining_core"][key])),&format!("member.mining_core.{key}"),Some("powder"),"member");json!({"type":kind,"reportedPowder":f(&format!("powder_{kind}")),"reportedTotal":f(&format!("powder_{kind}_total")),"spent":f(&format!("powder_spent_{kind}")),"available":f(&format!("powder_{kind}")),"counterRules":{"revision":TREE_RULES["counterRulesRevision"],"source":"SkyCrypt mining counter implementation","liveGameVerified":false}})}).collect::<Vec<_>>();
    let forge = at(m, "forge.forge_processes");
    let jobs=obj(forge).into_iter().take(16).flat_map(|(forge,v)|obj(&v).into_iter().take(64).map(move |(slot,v)|json!({"forge":forge,"slot":slot,"reported":evidence(&v,&format!("member.forge.forge_processes.{forge}.{slot}")),"completionTime":null,"readyToClaim":null,"reasonCode":"recipe_duration_and_modifiers_not_verified"}))).collect::<Vec<_>>();
    let objectives=obj(&m["objectives"]).into_iter().take(4096).filter(|(id,v)|re("^[A-Za-z0-9_: .-]{1,96}$",id)&&v.is_object()).map(|(id,v)|json!({"id":id,"status":text(&v["status"],96),"progress":nonnegative(&v["progress"]),"completedAt":nonnegative(&v["completed_at"]).filter(|n|*n!=0.),"evidence":evidence(&v,&format!("member.objectives.{id}"))})).collect::<Vec<_>>();
    let owned = at(m, "shards.owned");
    let shards=owned.as_array().map(|a|a.iter().take(2048).map(|v|json!({"id":text(&v["type"],96),"amount":nonnegative(&v["amount_owned"]),"lastCapturedAt":nonnegative(&v["captured"])})).collect::<Vec<_>>());
    let mut out = json!({});
    out["mining"] = partial(
        json!({"heartOfTheMountain":mining,"powders":powders,"coreOfTheMountain":mining["core"],"commissions":{"active":null,"dailyBonusesRemaining":null,"reasonCode":"not_exposed_in_verified_profile_fields"},"forgeJobs":jobs,"forgeCoverage":field(forge.clone(),"member.forge.forge_processes",None,"member"),"crystals":evidence(at(m,"mining_core.crystals"),"member.mining_core.crystals"),"nucleus":evidence(at(m,"mining_core.nucleus"),"member.mining_core.nucleus"),"goldCollection":field(json!(nonnegative(at(m,"collection.GOLD_INGOT"))),"member.collection.GOLD_INGOT",Some("items"),"member"),"coreEvidence":evidence(&m["mining_core"],"member.mining_core")}),
        has(m, "skill_tree") || has(m, "mining_core"),
        vec![],
    );
    out["foraging"] = partial(
        json!({"heartOfTheForest":foraging,"whispers":whispers(m),"centerOfTheForest":foraging["core"],"coreEvidence":evidence(&m["foraging_core"],"member.foraging_core"),"foragingEvidence":evidence(&m["foraging"],"member.foraging")}),
        has(m, "foraging_core") || has(m, "foraging"),
        vec![],
    );
    out["hunting"] = partial(
        json!({"shards":shards,"shardCoverage":field(owned.clone(),"member.shards.owned",None,"member"),"attributes":evidence(&m["attributes"],"member.attributes"),"traps":evidence(at(m,"shards.traps"),"member.shards.traps"),"fusion":evidence(at(m,"shards.fused"),"member.shards.fused"),"toolkitEvidence":evidence(at(m,"foraging.hunting_toolkit"),"member.foraging.hunting_toolkit"),"shardDefinitions":at(&DEFINITIONS,"shards.attributes"),"attributeLevelThresholds":at(&DEFINITIONS,"shards.attribute_levelling"),"effectiveStats":null,"attributeLevels":null,"reasonCode":"stack_counts_are_not_verified_effective_attribute_levels"}),
        has(m, "shards") || has(m, "attributes"),
        vec![],
    );
    out["dailies"] = partial(
        json!({"activities":activities(&merge(m.clone(),json!({"community_upgrades":raw["community_upgrades"]}))),"resetTimezone":"UTC day boundaries and per-activity resets remain unverified; no automatic completedToday inference.","liveModifiersUnknown":true}),
        true,
        vec![],
    );
    out["quests"] = partial(
        json!({"objectives":objectives,"objectiveCoverage":{"expected":obj(&m["objectives"]).len(),"returned":objectives.len(),"truncated":obj(&m["objectives"]).len()>4096},"quests":evidence(&m["quests"],"member.quests"),"unlockEvidence":evidence(&json!({"visitedZones":at(m,"player_data.visited_zones"),"visitedModes":at(m,"player_data.visited_modes"),"unlockedCollectionTiers":at(m,"player_data.unlocked_coll_tiers")}),"member.player_data"),"levelingEvidence":evidence(&m["leveling"],"member.leveling"),"communityUpgrades":evidence(&raw["community_upgrades"],"profile.community_upgrades")}),
        has(m, "objectives") || has(m, "quests"),
        vec![],
    );
    for (section, path) in [
        ("rift", "rift"),
        ("crimson_isle", "nether_island_player_data"),
    ] {
        out[section] = partial(
            json!({"progress":evidence(&m[path],&format!("member.{path}"))}),
            has(m, path),
            vec![],
        )
    }
    out["chocolate_factory"] = partial(
        json!({"progress":evidence(at(m,"events.easter"),"member.events.easter"),"chocolate":field(json!(nonnegative(at(m,"events.easter.chocolate"))),"member.events.easter.chocolate",Some("chocolate"),"member"),"productionPerSecond":null,"reasonCode":"live_production_formula_not_verified"}),
        has(m, "events.easter"),
        vec![],
    );
    out["systems"] = partial(
        json!({"registry":[],"bestiary":evidence(&m["bestiary"],"member.bestiary"),"reportedCombatRecords":field(Value::Object(obj(&m["player_stats"]).into_iter().filter(|(k,v)|re("^[A-Za-z0-9_: .-]{1,96}$",k)&&n(v).is_some()).collect()),"member.player_stats",None,"member"),"reportedSystemFields":obj(m).keys().filter(|k|re("^[A-Za-z0-9_: .-]{1,96}$",k)&&!re("(?i)^(?:uuid|player_uuid|player_id|display_name|teammates|participants|members|session|sessions|last_login|last_logout|first_join|chat|messages|data|email|token|access_token|started_by|claimed_by|who_started)$",k)).collect::<Vec<_>>(),"unsupportedViews":["persistent_user_facts","goal_tracking","recommendations","money_method_comparison","continuous_API_session_history"]}),
        true,
        vec![],
    );
    let registry=obj(&out).into_iter().filter(|(id,_)|id!="systems").map(|(id,s)|json!({"id":id,"status":s["status"],"source":"profile_API","gameScope":"member_unless_a_field_says_profile","fullyImplemented":false})).collect::<Vec<_>>();
    out["systems"]["data"]["registry"] = json!(registry);
    out
}
