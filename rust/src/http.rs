use crate::{mcp::Server, service::Service, types::now};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};
#[derive(Clone)]
struct Gate {
    service: Arc<Service>,
    slots: Arc<tokio::sync::Semaphore>,
    recent: Arc<Mutex<VecDeque<i64>>>,
    maximum: usize,
}
fn reply(status: u16, error: &str) -> Response {
    (
        axum::http::StatusCode::from_u16(status).unwrap(),
        Json(json!({"error":error})),
    )
        .into_response()
}
fn local_host(v: &str) -> bool {
    let authority = v.parse::<axum::http::uri::Authority>();
    authority.is_ok_and(|a| {
        ["localhost", "127.0.0.1", "[::1]", "::1"].contains(&a.host().to_lowercase().as_str())
    })
}
async fn guard(State(g): State<Gate>, request: Request, next: Next) -> Response {
    let started = Instant::now();
    let health = request.uri().path() == "/health";
    let mut response=async{
 if !request.headers().get("host").and_then(|v|v.to_str().ok()).is_some_and(local_host){return reply(403,"invalid_host")}
 if let Some(origin)=request.headers().get("origin"){let allowed=origin.to_str().ok().and_then(|s|s.parse::<axum::http::Uri>().ok()).is_some_and(|u|[Some("http"),Some("https")].contains(&u.scheme_str())&&u.authority().is_some_and(|a|local_host(a.as_str()))&&u.path()=="/");if !allowed{return reply(403,"invalid_origin")}}
 if health&&request.method()==axum::http::Method::GET{return (axum::http::StatusCode::OK,Json(json!({"status":"ok","mode":if g.service.upstream.source=="hypixel"{"live"}else{"demo"},"scope":"private_loopback","unaffiliatedWithHypixel":true}))).into_response()}
 if request.uri().path()!="/mcp"{return reply(404,"not_found")}
if request.method()!=axum::http::Method::POST{let mut response=reply(405,"method_not_allowed");response.headers_mut().insert("allow",axum::http::HeaderValue::from_static("POST"));return response}
if !request.headers().get("content-type").and_then(|v|v.to_str().ok()).is_some_and(|s|s.to_lowercase().starts_with("application/json")){return reply(415,"json_required")}
 let permit=g.slots.clone().try_acquire_owned();let throttled={let mut recent=g.recent.lock().unwrap();let t=now();while recent.front().is_some_and(|n|*n<=t-60000){recent.pop_front();}
if permit.is_err()||recent.len()>=120{true}else{recent.push_back(t);false}};if throttled{let mut response=reply(429,"local_rate_limit");response.headers_mut().insert("retry-after",axum::http::HeaderValue::from_static("60"));return response}
let _permit=permit.unwrap();if request.headers().get("content-length").and_then(|v|v.to_str().ok()).and_then(|s|s.parse::<usize>().ok()).is_some_and(|n|n>64*1024){return reply(413,"body_too_large")}
let (parts,body)=request.into_parts();let bytes=match to_bytes(body,64*1024).await{Ok(bytes)=>bytes,Err(_)=>return reply(400,"invalid_or_oversized_json")};if serde_json::from_slice::<serde_json::Value>(&bytes).is_err(){return reply(400,"invalid_or_oversized_json")}next.run(Request::from_parts(parts,Body::from(bytes))).await
 }.await;
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    if !health {
        g.service.upstream.diagnostics.http(
            response.status().as_u16(),
            started.elapsed().as_secs_f64() * 1000.,
            g.maximum - g.slots.available_permits(),
        );
    }
    response
}
pub fn router(service: Arc<Service>, max_active: usize) -> anyhow::Result<Router> {
    if !(1..=8).contains(&max_active) {
        anyhow::bail!("Concurrent requests must be between 1 and 8.")
    }
    let s = service.clone();
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true);
    let transport = StreamableHttpService::new(
        move || Ok(Server { service: s.clone() }),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let gate = Gate {
        service,
        slots: Arc::new(tokio::sync::Semaphore::new(max_active)),
        recent: Arc::new(Mutex::new(VecDeque::new())),
        maximum: max_active,
    };
    Ok(Router::new()
        .nest_service("/mcp", transport)
        .fallback(|| async { reply(404, "not_found") })
        .layer(middleware::from_fn_with_state(gate, guard)))
}
