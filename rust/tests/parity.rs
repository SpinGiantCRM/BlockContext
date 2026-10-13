use blockcontext::{
    activities, bridge, client, factual, inventory, normalize, progression, systems, upstream,
};
use serde_json::Value;
fn compare(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-10 * a.abs().max(b.abs()).max(1.),
                "{path}: {a} != {b}"
            );
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.len(),
                b.len(),
                "{path}: different fields {:?} {:?}",
                a.keys(),
                b.keys()
            );
            for (k, v) in a {
                compare(
                    v,
                    b.get(k).unwrap_or_else(|| panic!("{path}: missing {k}")),
                    &format!("{path}.{k}"),
                );
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: different array lengths");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
#[test]
fn synthetic_reference_parity() {
    let corpus = std::env::var_os("BLOCKCONTEXT_PARITY_FILE")
        .map(|path| std::fs::read_to_string(path).unwrap())
        .unwrap_or_else(|| include_str!("../../fixtures/contracts/rust-parity.json").to_owned());
    let cases: Value = serde_json::from_str(&corpus).unwrap();
    for (i, c) in cases.as_array().unwrap().iter().enumerate() {
        let a = &c["args"];
        let result = match c["name"].as_str().unwrap() {
            "map_fields" => factual::map_fields(
                &a[0],
                a[1].as_str().unwrap(),
                a[2].as_str().unwrap(),
                a[3].as_u64().unwrap() as usize,
            ),
            "project_own" => factual::project_own(&a[0], 0).unwrap(),
            "project_profile" => upstream::project_profile(&a[0], a[1].as_str().unwrap()).unwrap(),
            "bridge_project" => serde_json::from_slice::<Value>(
                &bridge::project(
                    &serde_json::to_vec(&a[0]).unwrap(),
                    a[1].as_str().unwrap(),
                    a[2].as_i64().unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
            "game_evidence" => factual::game_evidence(&a[0]),
            "pet_progress" => progression::pet_progress(&a[0], &a[1], &a[2]),
            "dungeon_progress" => progression::dungeon_progress(&a[0]),
            "pets_section" => progression::pets_section(&a[0], a[1].as_bool().unwrap()),
            "dungeons" => progression::dungeons(&a[0], a[1].as_str()),
            "skills" => progression::skills(&a[0], &a[1], &a[2]),
            "greenhouse" => progression::greenhouse(&a[0]),
            "garden" => progression::garden(&a[0]),
            "collections" => progression::collections(&a[0], &a[1]),
            "slayers" => progression::slayers(&a[0]),
            "buffs" => progression::buffs(&a[0]),
            "whispers" => systems::whispers(&a[0]),
            "game_systems" => systems::game_systems(&a[0], &a[1]),
            "rule_number" => systems::rule_number(
                &a[0],
                &a[1]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_f64().unwrap()))
                    .collect(),
            ),
            "stored_weapon" => inventory::stored_weapon(&a[0]),
            "item_details" => inventory::item_details(&a[0], &a[1]),
            "project_items" => inventory::project_items(&a[0]).unwrap(),
            "variants" => inventory::variants(a[0].as_str().unwrap(), &a[1]),
            "magical_power" => inventory::magical_power(&a[0], &a[1]),
            "accessory_diagnostics" => inventory::accessory_diagnostics(&a[0], &a[1]),
            "schema" => {
                client::validate_contract(a[0].as_str().unwrap(), &a[1]).unwrap_or(Value::Null)
            }
            "reset_assessment" => activities::reset_assessment(&a[0], a[1].as_i64().unwrap()),
            "reconcile" => {
                serde_json::json!(activities::reconcile(&a[0], &a[1], a[2].as_i64().unwrap()))
            }
            "normalize" => {
                normalize::normalize(&a[0], a[1].as_str().unwrap(), &a[2], &a[3], &a[4], &a[5])
                    .unwrap()
            }
            "decode" => inventory::decode(&a[0]).unwrap(),
            "backpacks" => inventory::backpacks(&a[0], a[1].as_bool().unwrap()),
            "loadouts" => {
                inventory::loadouts(&a[0], a[1].as_bool().unwrap(), a[2].as_str().unwrap())
            }
            other => panic!("Unmapped fixture {other}"),
        };
        compare(&result, &c["result"], &format!("Case {i} {}", c["name"]));
    }
}
