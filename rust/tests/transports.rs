use blockcontext::{
    cache::Cache, client::Client, diagnostics::Diagnostics, http, service::Service,
    types::SECTIONS, upstream::Upstream,
};
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
fn call(name: &str, args: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name.to_owned()).with_arguments(args.as_object().unwrap().clone())
}
#[tokio::test]
async fn official_http_client_discovers_and_reads_every_paginated_section() {
    let diagnostics = Arc::new(Diagnostics::new(None));
    let service = Service::new(
        Upstream::new("demo", String::new(), None, 30, diagnostics).unwrap(),
        Cache::open(Path::new(":memory:")).unwrap(),
        Client::default(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = http::router(service, 2).unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let transport = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let client = ().serve(transport).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 8);
    assert_eq!(
        tools
            .iter()
            .filter(|t| t
                .annotations
                .as_ref()
                .is_some_and(|a| a.read_only_hint == Some(true)))
            .count(),
        7
    );
    for section in SECTIONS {
        let result=client.call_tool(call("skyblock_profile_section",json!({"player":"DemoPlayer","profile":"Strawberry","section":section,"mode":"snapshot","offset":1,"limit":3}))).await.unwrap();
        assert_ne!(
            result.is_error,
            Some(true),
            "{section}: {:?}",
            result.content
        );
        assert_eq!(result.structured_content.unwrap()["section"], *section);
    }
    let market = client
        .call_tool(call(
            "skyblock_market_quote",
            json!({"item":"WHEAT","quantity":2.0}),
        ))
        .await
        .unwrap();
    assert_eq!(
        market.structured_content.unwrap()["bazaar"]["instantBuy"]["totalCoins"].as_f64(),
        Some(16.)
    );
    let auction = client
        .call_tool(call("skyblock_auction_quote", json!({"item":"SWORD"})))
        .await
        .unwrap();
    assert_eq!(
        auction.structured_content.unwrap()["coverage"]["status"],
        "complete"
    );
    client.cancel().await.unwrap();
    server.abort();
}
#[tokio::test]
async fn official_stdio_client_loads_absolute_private_env_and_keeps_logs_private() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("private.env");
    let logs = dir.path().join("usage.jsonl");
    // Forward slashes are valid on Windows and avoid dotenv escape sequences.
    let db_path = dir
        .path()
        .join("cache.sqlite")
        .to_string_lossy()
        .replace('\\', "/");
    let log_path = logs.to_string_lossy().replace('\\', "/");
    std::fs::write(&path,format!("BLOCKCONTEXT_MODE=demo\nBLOCKCONTEXT_TRANSPORT=stdio\nBLOCKCONTEXT_DB={db_path}\nBLOCKCONTEXT_LOG_PATH={log_path}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_blockcontext"));
    command
        .arg("--env-file")
        .arg(&path)
        .stderr(std::process::Stdio::inherit())
        .current_dir(dir.path())
        .env_remove("BLOCKCONTEXT_MODE")
        .env_remove("BLOCKCONTEXT_TRANSPORT")
        .env_remove("BLOCKCONTEXT_DB")
        .env_remove("BLOCKCONTEXT_LOG_PATH");
    let transport = TokioChildProcess::new(command).unwrap();
    let client = ().serve(transport).await.unwrap();
    assert_eq!(client.list_all_tools().await.unwrap().len(), 8);
    let result = client
        .call_tool(call(
            "skyblock_profile_context",
            json!({"player":"DemoPlayer","profile":"Strawberry"}),
        ))
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap()["metadata"]["source"],
        "synthetic_demo"
    );
    let config=client.call_tool(call("skyblock_poll_schedule",json!({"player":"0123456789abcdef0123456789abcdef","profile":"Apple","operation":"set","id":"hotm","target":"hotm_bonus_commissions","cadence":{"kind":"hotm_reset_reference"}}))).await.unwrap();
    assert_eq!(config.structured_content.unwrap()["apiRequests"], 0);
    client.cancel().await.unwrap();
    let logs = std::fs::read_to_string(logs).unwrap();
    assert!(!logs.contains("DemoPlayer"));
    assert!(!logs.contains("Strawberry"));
    assert!(!logs.contains("0123456789abcdef"));
}
