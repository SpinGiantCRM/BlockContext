use blockcontext::{activities, bridge, client, experiments};
use serde_json::{Value, json};
#[test]
fn api_counts_are_typed_without_inventing_daily_uses_or_charge_resets() {
    let v = experiments::api_usage(
        &json!({"pairings":{"claims_0":7,"claims_1":10,"claims_2":-1,"claims_3":true,"claims_4":0.5,"attempts_1":4,"claimed":true},"charge_track_timestamp":1792120424801_u64,"claims_resets":2}),
    );
    assert_eq!(v["games"][0]["reportedClaimCountSubtotal"], 17);
    assert_eq!(
        v["games"][0]["reportedClaimsByTier"],
        json!({"claims_0":7,"claims_1":10})
    );
    assert!(v["games"][1]["reportedClaimCountSubtotal"].is_null());
    assert!(v["usesToday"].is_null());
    assert!(v["currentCharges"].is_null());
    assert_eq!(v["exactResetTimeKnown"], false);
}
fn ledger(t: i64) -> Value {
    json!({"ledgerCreatedAt":t-1000,"persistentObservedChargeDecreases":2,"persistentObservedChargeIncreases":1,"observationGaps":1,"lastObservedCharges":1,"lastChargesObservedAt":t-10,"restoredFromDisk":true,"persistenceAvailable":true,"completeDailyCount":false})
}
fn client_view(t: i64, current: bool) -> Value {
    json!({"observations":{"experimentTable":{"values":{"availableCharges":0,"maximumCharges":3,"nextChargeSecondsAtObservation":900,"paidResetsRemainingToday":2},"observedAt":"2026-10-16T00:00:00.000Z","currentStateVerified":current},"experimentTracking":{"values":ledger(t),"observedAt":"2026-10-16T00:00:00.000Z","currentStateVerified":false}}})
}
#[test]
fn displayed_charges_and_persistent_decreases_are_separate_from_completed_experiments() {
    let a = json!({"activityId":"experimentation","status":"partial","evidence":{"value":{"pairings":{"claims_0":7}}}});
    for current in [true, false] {
        let r = activities::reconcile(&json!([a]), &client_view(100000, current), 100000).remove(0);
        assert_eq!(
            r["remainingCharges"],
            if current { json!(0) } else { Value::Null }
        );
        assert!(r["completedToday"].is_null());
        assert!(r["usesToday"].is_null());
        assert!(r["nextReset"].is_null());
        assert_eq!(
            r["chargeTracking"]["values"]["persistentObservedChargeDecreases"],
            2
        );
        assert_eq!(r["clientUsage"]["nextChargeSecondsAtObservation"], 900);
        assert_eq!(r["chargeTracking"]["completeDailyUseCount"], false);
    }
}
#[test]
fn client_and_bridge_validate_sources_bounds_and_preserve_only_the_experiment_ledger_when_old() {
    let t = 1792120424801_i64;
    let record = |values: Value, source: &str, stamp: i64| json!({"values":values,"readFrom":source,"observedAt":stamp});
    assert!(
        client::parse_observation(
            "experimentTable",
            &record(
                json!({"availableCharges":2,"maximumCharges":3}),
                "opened_menu",
                t
            )
        )
        .is_some()
    );
    for value in [
        json!({}),
        json!({"availableCharges":4,"maximumCharges":3}),
        json!({"availableCharges":1.5}),
        json!({"availableCharges":true}),
        json!({"nextChargeSecondsAtObservation":604801}),
    ] {
        assert!(
            client::parse_observation("experimentTable", &record(value, "opened_menu", t))
                .is_none()
        );
    }
    assert!(
        client::parse_observation(
            "experimentTable",
            &record(json!({"availableCharges":2}), "collector_ledger", t)
        )
        .is_none()
    );
    let good = ledger(t);
    assert!(
        client::parse_observation(
            "experimentTracking",
            &record(good.clone(), "collector_ledger", t)
        )
        .is_some()
    );
    let mut bad = good.clone();
    bad["lastChargesObservedAt"] = json!(t + 1);
    assert!(
        client::parse_observation("experimentTracking", &record(bad, "collector_ledger", t))
            .is_none()
    );
    assert!(
        client::parse_observation(
            "experimentTracking",
            &record(good.clone(), "opened_menu", t)
        )
        .is_none()
    );
    let id = "0123456789abcdef0123456789abcdef";
    let snapshot = json!({"schemaVersion":1,"collectorVersion":"0.3.7","exportedAt":t,"activeProfile":format!("{id}:apple"),"profiles":[{"uuid":id,"playerName":"DemoPlayer","profileName":"Apple","observations":{"experimentTracking":record(good,"collector_ledger",t),"experimentTable":record(json!({"availableCharges":2,"arbitrary":"PRIVATE_SENTINEL"}),"opened_menu",t)}}]});
    let bytes = bridge::project(&serde_json::to_vec(&snapshot).unwrap(), id, t).unwrap();
    assert!(
        !String::from_utf8(bytes.clone())
            .unwrap()
            .contains("PRIVATE_SENTINEL")
    );
    let old = bridge::project(
        &serde_json::to_vec(&snapshot).unwrap(),
        id,
        t + 2 * 86400000,
    )
    .unwrap();
    let old: Value = serde_json::from_slice(&old).unwrap();
    assert!(old["profiles"][0]["observations"]["experimentTracking"].is_object());
    assert!(
        old["profiles"][0]["observations"]
            .get("experimentTable")
            .is_none()
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snapshot.json");
    std::fs::write(&path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let r = client::Client { path: Some(path) }
        .query(
            id,
            Some("Apple"),
            Some("dailies"),
            false,
            10,
            0,
            50,
            t + 2 * 86400000,
        )
        .unwrap();
    assert!(r["observations"]["experimentTracking"].is_object());
    assert_eq!(
        r["observations"]["experimentTracking"]["currentStateVerified"],
        false
    );
    assert!(r["observations"].get("experimentTable").is_none());
}
