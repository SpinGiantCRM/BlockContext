use blockcontext::{
    cache::Cache, client::Client, diagnostics::Diagnostics, http, mcp::Server, service::Service,
    upstream::Upstream,
};
use clap::{Parser, Subcommand};
use rmcp::ServiceExt;
use std::{path::PathBuf, sync::Arc, time::Duration};
#[derive(Parser)]
#[command(
    version,
    about = "Self-hosted SkyBlock MCP; the default demo contains invented data."
)]
struct Args {
    #[arg(long)]
    env_file: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long,env="BLOCKCONTEXT_MODE",default_value="demo",value_parser=["live","demo"])]
    mode: String,
    #[arg(long,env="BLOCKCONTEXT_TRANSPORT",default_value="http",value_parser=["stdio","http"])]
    transport: String,
    #[arg(long, env = "PORT", default_value = "3008")]
    port: u16,
    #[arg(long, env = "BLOCKCONTEXT_DB", default_value = ".data/cache.sqlite")]
    db: PathBuf,
    #[arg(long, env = "BLOCKCONTEXT_MAX_CONCURRENT", default_value = "8")]
    max_concurrent: usize,
    #[arg(long, env = "BLOCKCONTEXT_LOG_PATH")]
    log_path: Option<PathBuf>,
    #[arg(long, env = "BLOCKCONTEXT_CLIENT_SNAPSHOT_PATH")]
    client_snapshot: Option<PathBuf>,
}
#[derive(Subcommand)]
enum Command {
    /// Upload passive owner snapshots over a verified SSH connection.
    Bridge {
        #[arg(long)]
        config: PathBuf,
    },
    /// Fixed-destination import helper for the operator's SSH connection.
    ImportSnapshot,
}
fn main() -> anyhow::Result<()> {
    let argv = std::env::args_os().collect::<Vec<_>>();
    let helper = argv.iter().any(|s| s == "import-snapshot" || s == "bridge");
    if !helper {
        let explicit = argv.iter().enumerate().find_map(|(i, s)| {
            let s = s.to_string_lossy();
            if let Some(s) = s.strip_prefix("--env-file=") {
                Some(PathBuf::from(s))
            } else if s == "--env-file" {
                argv.get(i + 1).map(PathBuf::from)
            } else {
                None
            }
        });
        if explicit.as_ref().is_some_and(|p| !p.is_absolute()) {
            anyhow::bail!("Use an absolute path for --env-file.")
        }
        let path = explicit.clone().unwrap_or_else(|| PathBuf::from(".env"));
        if explicit.is_some() || path.exists() {
            let blob = blockcontext::client::private_read(&path, 32768).map_err(|_| {
                anyhow::anyhow!("Cannot read the bounded private environment file.")
            })?;
            dotenvy::from_read(std::io::Cursor::new(blob))
                .map_err(|_| anyhow::anyhow!("Invalid private environment file."))?;
        }
    }
    let args = Args::parse();
    if matches!(args.command, Some(Command::ImportSnapshot)) {
        return blockcontext::bridge::import_fixed();
    }
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .enable_all()
        .build()?
        .block_on(run(args))
}
async fn run(args: Args) -> anyhow::Result<()> {
    if let Some(Command::Bridge { config }) = &args.command {
        return blockcontext::bridge::run(config).await;
    }
    if args.port == 0 {
        anyhow::bail!("PORT must be between 1 and 65535.")
    }
    let key = std::env::var("HYPIXEL_API_KEY").unwrap_or_default();
    if args.mode == "live" && key.is_empty() {
        anyhow::bail!("Live mode requires HYPIXEL_API_KEY in the private local environment.")
    }
    let limit = std::env::var("HYPIXEL_REQUESTS_PER_5_MINUTES")
        .unwrap_or_else(|_| "30".into())
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("Invalid Hypixel request budget."))?;
    let expiry = std::env::var("HYPIXEL_API_KEY_EXPIRES_AT")
        .ok()
        .filter(|s| !s.is_empty());
    let diagnostics = Arc::new(Diagnostics::new(args.log_path));
    let cache = Cache::open(&args.db)?;
    let upstream = Upstream::new(
        &args.mode,
        key,
        expiry.as_deref(),
        limit,
        diagnostics.clone(),
    )?;
    let service = Service::new(
        upstream,
        cache,
        Client {
            path: args.client_snapshot,
        },
    );
    diagnostics.lifecycle("started", &args.mode, &args.transport);
    diagnostics.resource();
    let metrics = diagnostics.clone();
    let measurements = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        tick.tick().await;
        loop {
            tick.tick().await;
            metrics.resource();
        }
    });
    if args.transport == "stdio" {
        let running = Server { service }.serve(rmcp::transport::stdio()).await?;
        tokio::select! {result=running.waiting()=>{result?;},_ = stop_signal()=>{}}
    } else {
        let router = http::router(service, args.max_concurrent)?;
        let listener =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, args.port)).await?;
        eprintln!(
            "BlockContext {} listening locally at http://127.0.0.1:{}/mcp",
            args.mode, args.port
        );
        if args.mode == "demo" {
            eprintln!("SYNTHETIC DEMO: invented sample values, not live player stats.")
        }
        axum::serve(listener, router)
            .with_graceful_shutdown(stop_signal())
            .await?;
    }
    measurements.abort();
    diagnostics.lifecycle("stopping", &args.mode, &args.transport);
    Ok(())
}
async fn stop_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Signal handler");
        tokio::select! {_ = tokio::signal::ctrl_c()=>{},_ = term.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
