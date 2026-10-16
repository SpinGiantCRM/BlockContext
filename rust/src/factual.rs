use crate::types::*;
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;
pub static ROOTS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fixtures/contracts/domain-roots.json")).unwrap()
});
pub static REQUIREMENTS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fixtures/contracts/requirements.json")).unwrap()
});
pub static DEFINITIONS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../fixtures/domain/systems-definitions.json"
    ))
    .unwrap()
});
pub static TREE_RULES: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../fixtures/domain/skill-trees.json")).unwrap()
});
static PRIVATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|_)(?:private|uuid|uniqueid|player_id|player_uuid|email|ip|token|secret|password|session|sessions|chat|messages|social_media|coop_invitation|deletion_notice|last_login|last_logout|first_login|first_join|participants|teammates|started_by|claimed_by|who_started|initiator|initiator_name|owner|owners|donator|originator)(?:$|_)").unwrap()
});
fn private(k: &str, v: &Value) -> bool {
    // This exact bestiary mob ID names a dungeon Secret Bat, not a credential.
    // Accept only a nonnegative safe integer; similarly named keys and text
    // values must still pass through the private-field filter.
    if k == "dungeon_secret_bat_1"
        && n(v).is_some_and(|v| v >= 0. && v.fract() == 0. && v <= 9007199254740991.)
    {
        return false;
    }
    !(n(v).is_some() && ["secret_count", "secrets_found", "token_count"].contains(&k))
        && PRIVATE.is_match(k)
}
pub fn stamp(v: &mut Value, t: i64) {
    match v {
        Value::Object(o) => {
            for (k, v) in o {
                if k == "observedAt" && v == "see_response_metadata.retrievedAt" {
                    *v = json!(iso(t));
                } else {
                    stamp(v, t);
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                stamp(v, t)
            }
        }
        _ => {}
    }
}
pub fn domain(path: &str) -> String {
    let p = path.strip_prefix("member.").unwrap_or(path);
    let mut best = (0, "other_game_systems");
    for (domain, roots) in ROOTS.as_object().unwrap() {
        for r in arr(roots) {
            if let Some(r) = r.as_str()
                && (p == r || p.starts_with(&format!("{r}.")))
                && r.len() > best.0
            {
                best = (r.len(), domain);
            }
        }
    }
    best.1.into()
}
pub fn unit(path: &str) -> Option<&'static str> {
    if re(r"(?:^|\.)enchantments\.[a-z0-9_]+$", path) {
        Some("enchantment level")
    } else if re(
        r"(?:coin_purse|bank_account|\.balance|\.amount_coins|coins_paid)$",
        path,
    ) {
        Some("SkyBlock coins")
    } else if re(r"(?:experience|\.xp|_xp|\.exp)$", path) {
        Some("XP")
    } else if re(
        r"(?:powder_(?:mithril|gemstone|glacite)(?:_total)?|powder_spent_(?:mithril|gemstone|glacite))$",
        path,
    ) {
        Some("powder")
    } else if re(r"(?:\.collection\.[A-Z0-9_:]+|\.amount_owned)$", path) {
        Some("items")
    } else if path.ends_with(".current") && path.contains(".essence.") {
        Some("essence")
    } else {
        None
    }
}
pub fn project_own(v: &Value, depth: usize) -> Result<Value> {
    if depth > 32 {
        return Err(AppError::new(
            "UPSTREAM_INVALID",
            "Game projection exceeds supported nesting depth.",
        ));
    }
    Ok(match v {
        Value::String(s) if uuid(s).is_some() || s.contains('@') => Value::Null,
        Value::Array(a) => Value::Array(
            a.iter()
                .map(|v| project_own(v, depth + 1))
                .collect::<Result<_>>()?,
        ),
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, v)| {
                    !private(k, v) && uuid(k).is_none() && re("^[A-Za-z0-9_: .+-]{1,96}$", k)
                })
                .map(|(k, v)| Ok((k.clone(), project_own(v, depth + 1)?)))
                .collect::<Result<_>>()?,
        ),
        _ => v.clone(),
    })
}
pub fn map_fields(v: &Value, prefix: &str, scope: &str, max: usize) -> Value {
    let _span = crate::diagnostics::current_span("field_map");
    struct Walker<'a> {
        fields: Vec<Value>,
        omissions: Vec<Value>,
        nodes: usize,
        leaves: usize,
        truncated: bool,
        scope: &'a str,
        max: usize,
    }
    impl Walker<'_> {
        fn omit(&mut self, path: &str, why: &str) {
            if self.omissions.len() < 128 {
                self.omissions.push(json!({"path":path,"reasonCode":why}))
            }
        }
        fn record(&mut self, path: &str, v: Value, extra: Value) {
            if self.fields.len() >= self.max {
                self.truncated = true;
                return;
            }
            self.fields.push(merge(json!({"path":path,"status":if v.is_null(){"unknown"}else{"known"},"value":v,"domain":domain(path),"scope":self.scope,"unit":unit(path),"observedAt":"see_response_metadata.retrievedAt","gameUpdatedAt":null}),extra));
        }
        fn visit(&mut self, v: &Value, path: &str, depth: usize) {
            self.nodes += 1;
            if self.nodes > 100000 || depth > 20 {
                self.truncated = true;
                self.omit(path, "projection_budget");
                return;
            }
            let key = path.rsplit('.').next().unwrap_or("");
            if private(key, v) || uuid(key).is_some() || re(r"^\d{1,3}(?:\.\d{1,3}){3}$", key) {
                self.omit(path, "identity_or_non_game_private_field");
                return;
            }
            if v.is_object()
                && v["data"]
                    .as_str()
                    .is_some_and(|s| re("^[A-Za-z0-9+/]+=*$", s))
            {
                if self.fields.len() < self.max {
                    self.leaves += 1;
                    self.record(path,Value::Null,json!({"status":"partial","valueType":"encoded_inventory","unit":null,"meaning":"Encoded item container; see storage.items and decoded equipment locations.","reasonCode":"encoded_inventory_not_exported"}));
                } else {
                    self.truncated = true
                }
                return;
            }
            if v.is_object() || v.is_array() {
                let entries: Vec<(String, &Value)> = match v {
                    Value::Object(o) => o.iter().map(|(k, v)| (k.clone(), v)).collect(),
                    Value::Array(a) => a
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (i.to_string(), v))
                        .collect(),
                    _ => vec![],
                };
                if entries.is_empty() {
                    self.leaves += 1;
                    self.record(path,v.clone(),json!({"valueType":if v.is_array(){"array"}else{"object"},"unit":null,"meaning":"Explicit empty game field; not inferred from absence."}));
                }
                for (k, v) in entries {
                    if re("^[A-Za-z0-9_: .+-]{1,96}$", &k) {
                        self.visit(v, &format!("{path}.{k}"), depth + 1)
                    } else {
                        self.omit(&format!("{path}.[invalid_key]"), "invalid_key")
                    }
                }
                return;
            }
            self.leaves += 1;
            if self.fields.len() >= self.max {
                self.truncated = true;
                return;
            }
            if let Value::String(s) = v
                && (uuid(s).is_some()
                    || s.contains('@')
                    || s.chars().count() > 512
                    || re("(?:Bearer |sk-[A-Za-z0-9]|-----BEGIN)", s))
            {
                self.omit(path, "identity_private_or_unbounded_text");
                return;
            }
            let u = unit(path);
            let kind = match v {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                _ => "string",
            };
            let value = if v.is_string() {
                json!(text(v, 512))
            } else {
                v.clone()
            };
            self.record(path,value,json!({"valueType":kind,"meaning":u.map(|u|format!("Reported {u} value.")).unwrap_or_else(||"Reported game field; exact unit or game semantics are not inferred.".into()),"semanticStatus":if u.is_some(){"unit_mapped"}else{"reported_field_semantics_unverified"},"reasonCode":if v.is_null(){json!("api_returned_null")}else{Value::Null}}));
        }
    }
    let mut w = Walker {
        fields: vec![],
        omissions: vec![],
        nodes: 0,
        leaves: 0,
        truncated: false,
        scope,
        max,
    };
    w.visit(v, prefix, 0);
    json!({"fields":w.fields,"coverage":{"returned":w.fields.len(),"visitedLeafFields":w.leaves,"omissions":w.omissions,"omissionsLimited":w.omissions.len()==128,"truncated":w.truncated,"completeGameFieldEnumeration":!w.truncated,"privateIdentitiesExcluded":true,"encodedInventoriesExported":false,"allSemanticsVerified":false}})
}
pub fn game_evidence(v: &Value) -> Value {
    fn visit(v: &Value, depth: usize, budget: &mut i32, truncated: &mut bool) -> Value {
        *budget -= 1;
        if *budget < 0 || depth > 10 {
            *truncated = true;
            return Value::Null;
        }
        match v {
            Value::String(s) => {
                if s.chars().count() > 128 {
                    *truncated = true
                }
                if uuid(s).is_some() {
                    Value::Null
                } else {
                    json!(text(v, 128))
                }
            }
            Value::Array(a) => {
                if a.len() > 2048 {
                    *truncated = true
                }
                json!(
                    a.iter()
                        .take(2048)
                        .map(|v| visit(v, depth + 1, budget, truncated))
                        .collect::<Vec<_>>()
                )
            }
            Value::Object(o) => {
                if o.len() > 2048 {
                    *truncated = true
                }
                Value::Object(o.iter().take(2048).filter(|(k,_)|re(r"^[A-Za-z0-9_: .-]{1,96}$",k)&&!re(r"(?i)^(?:uuid|player_uuid|player_id|display_name|teammates|participants|members|session|sessions|last_login|last_logout|first_join|chat|messages|data|email|token|access_token|started_by|claimed_by|who_started)$",k)&&uuid(k).is_none()).map(|(k,v)|(k.clone(),visit(v,depth+1,budget,truncated))).collect())
            }
            _ => v.clone(),
        }
    }
    let mut budget = 2048;
    let mut truncated = false;
    let value = visit(v, 0, &mut budget, &mut truncated);
    json!({"value":value,"truncated":truncated})
}
pub fn field(v: Value, source: &str, unit: Option<&str>, scope: &str) -> Value {
    json!({"status":if v.is_null(){"unknown"}else{"known"},"value":v,"unit":unit,"scope":scope,"source":source,"reasonCode":if v.is_null(){json!("not_exposed_or_not_verified")}else{Value::Null},"observedAt":"see_response_metadata.retrievedAt","gameUpdatedAt":null})
}
pub fn evidence(v: &Value, path: &str) -> Value {
    let e = game_evidence(v);
    let mut f = field(
        e["value"].clone(),
        path,
        None,
        if path.starts_with("profile.") {
            "shared_profile"
        } else {
            "member"
        },
    );
    if e["truncated"] == true {
        f["reasonCode"] = json!("evidence_projection_truncated");
        if !e["value"].is_null() {
            f["status"] = json!("partial")
        }
    }
    merge(
        f,
        json!({"truncated":e["truncated"],"interpretation":"Reported game fields; missing values are not zero, and undocumented counters and timestamps are not inferred to be current completions or unlocks."}),
    )
}
pub fn additional_sections(m: &Value, raw: &Value, metadata: &Value) -> Value {
    let mapped = map_fields(m, "member", "member", 12000);
    let shared = map_fields(
        &Value::Object(
            obj(raw)
                .into_iter()
                .filter(|(k, _)| {
                    ![
                        "members",
                        "profile_id",
                        "cute_name",
                        "selected",
                        "game_mode",
                    ]
                    .contains(&k.as_str())
                })
                .collect(),
        ),
        "profile",
        "shared_profile",
        12000,
    );
    let account = map_fields(metadata, "account", "account", 12000);
    let all = arr(&mapped["fields"])
        .into_iter()
        .chain(arr(&shared["fields"]))
        .chain(arr(&account["fields"]))
        .collect::<Vec<_>>();
    let mut out = json!({"api_fields":{"status":"partial","data":{"fields":all,"coverage":{"member":mapped["coverage"],"sharedProfile":shared["coverage"],"account":account["coverage"]},"newFieldsRetained":true},"warnings":["Every retained game field is mapped by source path, type and scope. Undocumented semantics stay unverified; identities and encoded inventory payloads are excluded."]}});
    for domain in [
        "fishing",
        "bestiary",
        "travel",
        "profile_upgrades",
        "events",
        "orders",
        "museum",
    ] {
        let fields = all
            .iter()
            .filter(|f| {
                f["domain"] == domain
                    || domain == "profile_upgrades"
                        && f["path"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("profile.community_upgrades"))
            })
            .cloned()
            .collect::<Vec<_>>();
        out[domain] = json!({"status":if fields.is_empty(){"unavailable"}else{"partial"},"data":{"fields":fields,"complete":false,"reasonCode":if fields.is_empty(){"not_exposed_in_profile_snapshot"}else{"reported_game_fields"}},"warnings":["No absence is interpreted as zero, empty holdings, locked or uncompleted. Separate endpoints and manually viewed menus may provide more evidence."]});
    }
    out
}
