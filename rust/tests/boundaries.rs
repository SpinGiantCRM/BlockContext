use blockcontext::{
    cache::{Cache, next_check},
    client,
    diagnostics::Diagnostics,
    http,
    mcp::Server,
    service::Service,
    types::{AppError, now},
    upstream::Upstream,
};
use serde_json::json;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tower::ServiceExt;
#[tokio::test]
async fn cache_coalesces_preserves_stale_and_blocks_expired_cached_polling() {
    let tick = Arc::new(AtomicI64::new(100000));
    let t = tick.clone();
    let cache = Cache::with_clock(
        Path::new(":memory:"),
        Arc::new(move || t.load(Ordering::SeqCst)),
    )
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tasks = vec![];
    for _ in 0..12 {
        let c = cache.clone();
        let n = calls.clone();
        tasks.push(tokio::spawn(async move {
            c.get(
                "snapshot".into(),
                1000,
                5000,
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Ok(json!({"safe":1}))
                },
                false,
            )
            .await
            .unwrap()
        }));
    }
    for t in tasks {
        assert_eq!(t.await.unwrap().value, json!({"safe":1}));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tick.store(102000, Ordering::SeqCst);
    let stale = cache
        .get(
            "snapshot".into(),
            1000,
            5000,
            async { Err(AppError::retry("UPSTREAM_TIMEOUT", "Timeout.", 15)) },
            false,
        )
        .await
        .unwrap();
    assert_eq!(stale.cache, "stale");
    tick.store(106000, Ordering::SeqCst);
    assert!(cache.peek("snapshot", 5000).is_none());
    let result = cache
        .snapshot(
            "player".into(),
            1000,
            5000,
            async { Ok(json!({"retrievedAt":106000})) },
            "snapshot",
        )
        .await
        .unwrap();
    assert_eq!(result.fetched_at, 106000);
    tick.store(112000, Ordering::SeqCst);
    let result = cache
        .snapshot(
            "player".into(),
            1000,
            5000,
            async { panic!("Cached polling must not fetch") },
            "cached",
        )
        .await
        .unwrap_err();
    assert_eq!(result.code, "SNAPSHOT_REFRESH_REQUIRED");
}
#[test]
fn sqlite_schedules_survive_reopen_and_use_conditional_completion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.sqlite");
    let tick = Arc::new(AtomicI64::new(100000));
    let t = tick.clone();
    let cache = Cache::with_clock(&path, Arc::new(move || t.load(Ordering::SeqCst))).unwrap();
    let owner = "hypixel:0123456789abcdef0123456789abcdef";
    cache
        .schedule_set(
            owner,
            "apple",
            "due",
            "currencies",
            &json!({"kind":"once","at":"1970-01-01T00:01:00.000Z"}),
        )
        .unwrap();
    drop(cache);
    let t = tick.clone();
    let cache = Cache::with_clock(&path, Arc::new(move || t.load(Ordering::SeqCst))).unwrap();
    let rows = cache.schedule_due(owner, Some("apple")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(cache.schedule_due(owner, Some("banana")).unwrap().len(), 0);
    assert!(!cache.schedule_complete(&rows[0], 50000).unwrap());
    assert!(cache.schedule_complete(&rows[0], 100000).unwrap());
    assert!(!cache.schedule_complete(&rows[0], 100000).unwrap());
    assert!(cache.schedule_due(owner, Some("apple")).unwrap().is_empty());
    let at = next_check(&json!({"kind":"hotm_reset_reference"}), 1791800000000).unwrap();
    assert_eq!(at % 86400000, 18000000);
    assert!(next_check(&json!({"kind":"bogus"}), 1000).is_err());
    assert!(next_check(&json!({"kind":"daily_utc","timeUtc":"25:10"}), 1000).is_err());
}
fn demo() -> Arc<Service> {
    let logs = Arc::new(Diagnostics::new(None));
    Service::new(
        Upstream::new("demo", String::new(), None, 30, logs).unwrap(),
        Cache::open(Path::new(":memory:")).unwrap(),
        client::Client::default(),
    )
}
#[tokio::test]
async fn http_boundaries_reject_foreign_hosts_origins_and_unbounded_bodies() {
    let app = http::router(demo(), 2).unwrap();
    let cases = vec![
        ("GET", "/health", "localhost", None, None, "", 200),
        ("GET", "/health", "attacker.example", None, None, "", 403),
        (
            "GET",
            "/health",
            "localhost",
            Some("https://attacker.example"),
            None,
            "",
            403,
        ),
        ("GET", "/mcp", "localhost", None, None, "", 405),
        ("POST", "/mcp", "localhost", None, None, "{}", 415),
        (
            "POST",
            "/mcp",
            "localhost",
            None,
            Some("application/json"),
            "{",
            400,
        ),
    ];
    for (method, path, host, origin, content, body, status) in cases {
        let mut builder = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header("host", host);
        if let Some(origin) = origin {
            builder = builder.header("origin", origin)
        }
        if let Some(content) = content {
            builder = builder.header("content-type", content)
        }
        let response = app
            .clone()
            .oneshot(builder.body(axum::body::Body::from(body)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
    }
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("host", "localhost")
        .header("content-type", "application/json")
        .header("content-length", 65537)
        .body(axum::body::Body::from(" ".repeat(65537)))
        .unwrap();
    assert_eq!(app.oneshot(request).await.unwrap().status().as_u16(), 413);
}
#[tokio::test]
async fn deferred_hotm_check_latches_only_on_explicit_snapshot() {
    let service = demo();
    let server = Server {
        service: service.clone(),
    };
    let id = "0123456789abcdef0123456789abcdef";
    let set=server.execute("skyblock_poll_schedule",json!({"player":id,"profile":"Strawberry","operation":"set","id":"hotm","target":"hotm_bonus_commissions","cadence":{"kind":"once","at":blockcontext::types::iso(now()-100)}})).await.unwrap();
    assert_eq!(set["apiRequests"], 0);
    let client = server
        .execute(
            "skyblock_live_context",
            json!({"player":id,"profile":"Strawberry"}),
        )
        .await
        .unwrap();
    assert_eq!(client["apiRequests"], 0);
    let cached = server
        .execute(
            "skyblock_profile_section",
            json!({"player":id,"profile":"Strawberry","section":"currencies","mode":"cached"}),
        )
        .await
        .unwrap();
    assert_eq!(cached["scheduledChecks"], json!([]));
    let snapshot = server
        .execute(
            "skyblock_profile_section",
            json!({"player":id,"profile":"Strawberry","section":"currencies","mode":"snapshot"}),
        )
        .await
        .unwrap();
    assert_eq!(
        snapshot["scheduledChecks"][0]["status"],
        "needs_client_observation"
    );
    assert_eq!(snapshot["scheduledChecks"][0]["additionalApiRequests"], 0);
    assert_eq!(
        snapshot["scheduledChecks"][0]["activity"]["remainingCharges"],
        serde_json::Value::Null
    );
    assert_eq!(
        service
            .cache
            .schedule_due(&format!("synthetic_demo:{id}"), Some("strawberry"))
            .unwrap()
            .len(),
        1
    );
    let error = server
        .execute(
            "skyblock_profile_context",
            json!({"player":id,"mode":"live"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "API_LIVE_POLLING_DISABLED");
}
#[test]
fn client_contracts_reject_conflicting_claims_private_attributes_and_future_ledger() {
    assert!(
        client::validate_contract(
            "commissions",
            &json!({"remainingBonusCommissions":0,"dailyBonusAvailable":true})
        )
        .is_none()
    );
    let v = json!({"slot":0,"id":"PET","count":1,"enchantments":{},"structuredAttributes":{"fields":[{"path":"owner.uuid","value":"private","semanticStatus":"reported_attribute"}],"omitted":0,"truncated":false,"complete":false}});
    assert!(client::validate_contract("item", &v).is_none());
    let unknown = json!({"slot":0,"id":"SAFE","count":1,"enchantments":{},"secret":"must strip"});
    let projected = client::validate_contract("item", &unknown).unwrap();
    assert!(projected.get("secret").is_none());
    let ledger = json!({"ledgerCreatedAt":1000,"persistentObservedClaims":2,"persistentObservedBonusClaims":2,"observationGaps":1,"restoredFromDisk":true,"persistenceAvailable":true,"completeDailyCount":false,"referenceDayStart":0,"observedBonusClaimsInReferenceDay":2,"referenceResetUtcHour":5,"referenceResetServerVerified":false,"expectedNextResetAt":86400000});
    let record = json!({"readFrom":"collector_ledger","observedAt":500,"values":ledger});
    assert!(client::parse_observation("commissionTracking", &record).is_none());
}

#[test]
fn bridge_rejects_other_owners_duplicates_and_keeps_only_projected_game_fields() {
    let owner = "0123456789abcdef0123456789abcdef";
    let t = 1000000;
    let mut snapshot = json!({"schemaVersion":1,"collectorVersion":"0.3.6","exportedAt":t,"activeProfile":format!("{owner}:apple"),"observedNow":["cookie"],"profiles":[{"uuid":owner,"playerName":"DemoPlayer","profileName":"Apple","observations":{"cookie":{"observedAt":t,"readFrom":"tab_display","values":{"active":true,"remainingSecondsAtObservation":10,"secret":"strip"}}}}]});
    let projected: serde_json::Value = serde_json::from_slice(
        &blockcontext::bridge::project(snapshot.to_string().as_bytes(), owner, t).unwrap(),
    )
    .unwrap();
    assert!(
        projected["profiles"][0]["observations"]["cookie"]["values"]
            .get("secret")
            .is_none()
    );
    snapshot["profiles"][0]["uuid"] = json!("fedcba9876543210fedcba9876543210");
    assert!(blockcontext::bridge::project(snapshot.to_string().as_bytes(), owner, t).is_err());
    snapshot["profiles"][0]["uuid"] = json!(owner);
    let duplicate = snapshot["profiles"][0].clone();
    snapshot["profiles"].as_array_mut().unwrap().push(duplicate);
    assert!(blockcontext::bridge::project(snapshot.to_string().as_bytes(), owner, t).is_err());
}

#[tokio::test]
async fn floating_integer_json_arguments_do_not_silently_fall_back_to_defaults() {
    let server = Server { service: demo() };
    let quote = server
        .execute(
            "skyblock_market_quote",
            json!({"item":"WHEAT","quantity":2.0}),
        )
        .await
        .unwrap();
    assert_eq!(quote["quantity"], 2);
    assert_eq!(
        quote["bazaar"]["instantBuy"]["totalCoins"].as_f64(),
        Some(16.)
    );
    let section=server.execute("skyblock_profile_section",json!({"player":"DemoPlayer","profile":"Strawberry","section":"collections","offset":1.0,"limit":2.0})).await.unwrap();
    assert_eq!(section["pagination"]["collected"]["offset"], 1);
    assert_eq!(section["pagination"]["collected"]["limit"], 2);
}

#[test]
fn profile_projection_handles_absent_treasures_and_excludes_other_members() {
    let owner = "0123456789abcdef0123456789abcdef";
    let other = "fedcba9876543210fedcba9876543210";
    let raw = json!({"profile_id":other,"members":{owner:{"dungeons":{},"coin_purse":42},other:{"coin_purse":999}},"cute_name":"Apple"});
    let result = blockcontext::upstream::project_profile(&raw, owner).unwrap();
    assert_eq!(result["members"].as_object().unwrap().len(), 1);
    assert_eq!(
        result["members"][owner]["dungeons"]["treasures"],
        json!({"runs":{},"chests":{}})
    );
    assert_eq!(
        result["members"][owner]["dungeons"]["dungeon_types"],
        json!({})
    );
}

#[tokio::test]
async fn upstream_expiry_budget_headers_and_redirects_fail_closed() {
    use axum::{Router, response::IntoResponse, routing::get};
    let calls = Arc::new(AtomicUsize::new(0));
    let n = calls.clone();
    let app = Router::new()
        .route(
            "/ok",
            get(move || {
                let n = n.clone();
                async move {
                    n.fetch_add(1, Ordering::SeqCst);
                    (
                        [("ratelimit-remaining", "2"), ("ratelimit-reset", "300")],
                        axum::Json(json!({"success":true})),
                    )
                }
            }),
        )
        .route(
            "/redirect",
            get(|| async {
                (axum::http::StatusCode::FOUND, [("location", "/ok")]).into_response()
            }),
        )
        .route("/malformed", get(|| async { "not json" }))
        .route(
            "/throttled",
            get(|| async {
                (
                    axum::http::StatusCode::TOO_MANY_REQUESTS,
                    [("retry-after", "30")],
                )
                    .into_response()
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let make = |expiry| {
        Upstream::new(
            "live",
            "synthetic-test-credential".into(),
            expiry,
            30,
            Arc::new(Diagnostics::new(None)),
        )
        .unwrap()
    };
    let expired = make(Some("2000-01-01T00:00:00Z"));
    assert_eq!(
        expired
            .get(&format!("{base}/ok"), true, "profiles")
            .await
            .unwrap_err()
            .code,
        "HYPIXEL_KEY_EXPIRED"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let up = make(None);
    assert_eq!(
        up.get(&format!("{base}/ok"), true, "profiles")
            .await
            .unwrap()["success"],
        true
    );
    assert_eq!(
        up.get(&format!("{base}/ok"), true, "profiles")
            .await
            .unwrap_err()
            .code,
        "RATE_LIMITED"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let up = make(None);
    assert_eq!(
        up.get(&format!("{base}/redirect"), false, "skills")
            .await
            .unwrap_err()
            .code,
        "UPSTREAM_UNAVAILABLE"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        up.get(&format!("{base}/malformed"), false, "skills")
            .await
            .unwrap_err()
            .code,
        "UPSTREAM_INVALID"
    );
    assert_eq!(
        up.get(&format!("{base}/throttled"), true, "profiles")
            .await
            .unwrap_err()
            .code,
        "RATE_LIMITED"
    );
    assert_eq!(
        up.get(&format!("{base}/ok"), true, "profiles")
            .await
            .unwrap_err()
            .code,
        "RATE_LIMITED"
    );
    server.abort();
}

#[test]
fn typed_client_objects_keep_contract_order_for_field_pagination() {
    let schema = json!({"type":"object","properties":{"first":{"type":"number"},"second":{"type":"number"}},"additionalProperties":false});
    let input: serde_json::Value =
        serde_json::from_str(r#"{"second":2,"ignored":0,"first":1}"#).unwrap();
    let projected = client::project_schema(&input, &schema, 0).unwrap();
    assert_eq!(
        projected
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

#[test]
fn native_diagnostics_record_bounded_stages_and_runtime_without_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.jsonl");
    let diag = Diagnostics::new(Some(path.clone()));
    {
        let _span = diag.span("cache_read");
        std::thread::sleep(Duration::from_millis(1));
    }
    {
        let _span = diag.span("untrusted-player-or-key");
    }
    diag.resource();
    let text = std::fs::read_to_string(path).unwrap();
    assert!(!text.contains("untrusted-player-or-key"));
    let rows = text
        .lines()
        .map(|s| serde_json::from_str::<serde_json::Value>(s).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        assert_eq!(row["schema"], 3);
        assert_eq!(row["runtime"], "rust");
        assert!(row["processStartedAt"].is_string());
        assert_eq!(row["runtimeVersion"], env!("CARGO_PKG_VERSION"));
    }
    assert_eq!(rows[0]["stage"], "cache_read");
    assert!(rows[0]["durationsMs"][0].as_f64().unwrap() > 0.);
}

#[test]
fn stage_sampling_is_bounded_and_reports_omitted_measurements() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.jsonl");
    let diag = Diagnostics::new(Some(path.clone()));
    for _ in 0..1500 {
        let _span = diag.span("cache_read");
    }
    assert!(
        !path.exists(),
        "Stage sampling must not write on the request path"
    );
    diag.resource();
    let text = std::fs::read_to_string(path).unwrap();
    let row: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(row["calls"], 1500);
    assert_eq!(row["omittedSamples"], 476);
    assert_eq!(row["durationsMs"].as_array().unwrap().len(), 1024);
}

#[test]
fn diagnostic_scopes_restore_after_nested_work_and_panics() {
    use blockcontext::diagnostics::{current_count, scoped};
    let dir = tempfile::tempdir().unwrap();
    let a = Arc::new(Diagnostics::new(Some(dir.path().join("a.jsonl"))));
    let b = Arc::new(Diagnostics::new(Some(dir.path().join("b.jsonl"))));
    scoped(a.clone(), || {
        current_count("cache_hits", 1);
        scoped(b.clone(), || current_count("cache_hits", 2));
        current_count("cache_hits", 1);
    });
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scoped(a.clone(), || panic!("synthetic panic"))
    }));
    current_count("cache_hits", 999);
    a.count("private-player-or-key", 123);
    a.resource();
    b.resource();
    for (file, expected) in [("a.jsonl", 2), ("b.jsonl", 2)] {
        let text = std::fs::read_to_string(dir.path().join(file)).unwrap();
        assert!(!text.contains("private-player-or-key"));
        let row: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(row["values"]["cache_hits"], expected);
    }
}

#[test]
fn inventory_failure_telemetry_has_sizes_and_codes_without_contents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.jsonl");
    let d = Arc::new(Diagnostics::new(Some(path.clone())));
    blockcontext::diagnostics::scoped(d.clone(), || {
        assert!(blockcontext::inventory::decode_nbt("private-content", 1024, 1024).is_err());
    });
    d.resource();
    let text = std::fs::read_to_string(path).unwrap();
    assert!(!text.contains("private-content"));
    let row: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(row["values"]["inventory_decode_failures"], 1);
    assert_eq!(row["values"]["inventory_input_bytes"], 15);
}

#[test]
fn bridge_lifecycle_reports_fixed_component_mode_and_transport() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("usage.jsonl");
    let d = Diagnostics::new(Some(p.clone())).for_bridge();
    d.lifecycle("started", "bridge", "ssh");
    let row: serde_json::Value =
        serde_json::from_str(std::fs::read_to_string(p).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!(row["component"], "bridge");
    assert_eq!(row["mode"], "bridge");
    assert_eq!(row["transport"], "ssh");
}
