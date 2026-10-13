use crate::types::*;
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
    sync::Mutex,
    time::Instant,
};
pub const TOOLS: &[&str] = &[
    "skyblock_profile_context",
    "skyblock_profile_section",
    "skyblock_market_quote",
    "skyblock_auction_quote",
    "skyblock_client_context",
    "skyblock_live_context",
    "skyblock_reference",
    "skyblock_poll_schedule",
];
pub const ENDPOINTS: &[&str] = &[
    "identity",
    "profiles",
    "skills",
    "items",
    "rank",
    "garden",
    "bazaar",
    "auctions",
    "collections",
    "museum",
    "bingo",
    "own_auctions",
    "election",
    "news",
    "fire_sales",
    "ended_auctions",
];
#[derive(Default)]
struct State {
    dropped: u64,
    last_failure: i64,
    last_cleanup: i64,
    file_started: Option<i64>,
    counters: std::collections::BTreeMap<&'static str, u64>,
    stages: std::collections::BTreeMap<&'static str, StageSamples>,
}
#[derive(Default)]
struct StageSamples {
    values: Vec<f64>,
    calls: u64,
    total: f64,
    maximum: f64,
}
pub const STAGES: &[&str] = &[
    "cache_read",
    "cache_write",
    "profile_normalize",
    "client_read",
    "rpc_payload_serialize",
    "cache_lock_wait",
    "sqlite_read",
    "sqlite_write",
    "cache_json_parse",
    "cache_json_serialize",
    "cpu_queue_wait",
    "worker_queue_wait",
    "field_map",
    "inventory_decode",
    "gzip_expand",
    "nbt_validate",
    "nbt_parse",
    "client_file_read",
    "client_validate",
    "upstream_budget_wait",
    "upstream_headers",
    "upstream_body",
    "upstream_json_parse",
    "auction_decode",
    "bridge_project",
    "bridge_ssh",
    "resource_tick_lag",
];
fn duration(ms: f64) -> f64 {
    if ms.is_finite() {
        (ms.clamp(0., 1e9) * 1000.).round() / 1000.
    } else {
        0.
    }
}
pub const COUNTERS: &[&str] = &[
    "cache_hits",
    "cache_misses",
    "cache_shared_loads",
    "cache_cooldowns",
    "cache_stale_fallbacks",
    "cache_expired_removed",
    "cache_capacity_removed",
    "cache_read_bytes",
    "cache_write_bytes",
    "upstream_requests",
    "upstream_body_bytes",
    "inventory_input_bytes",
    "inventory_expanded_bytes",
    "inventory_decode_failures",
    "client_snapshot_bytes",
    "bridge_unchanged",
    "bridge_uploads",
    "bridge_upload_bytes",
    "bridge_upload_failures",
    "bridge_snapshot_failures",
    "bridge_waiting",
];
thread_local! { static CURRENT: std::cell::RefCell<Option<std::sync::Arc<Diagnostics>>> = const { std::cell::RefCell::new(None) }; }
struct Restore(Option<std::sync::Arc<Diagnostics>>);
impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT.with(|v| {
            *v.borrow_mut() = self.0.take();
        });
    }
}
/// Only synchronous work can use this scope; it never crosses an await boundary.
pub fn scoped<T>(diagnostics: std::sync::Arc<Diagnostics>, work: impl FnOnce() -> T) -> T {
    if diagnostics.path.is_none() {
        return work();
    }
    let _restore = Restore(CURRENT.with(|v| v.replace(Some(diagnostics))));
    work()
}
pub struct OwnedSpan {
    diagnostics: std::sync::Arc<Diagnostics>,
    name: &'static str,
    started: Instant,
}
impl Drop for OwnedSpan {
    fn drop(&mut self) {
        self.diagnostics
            .measure(self.name, self.started.elapsed().as_secs_f64() * 1000.);
    }
}
pub fn current_span(name: &'static str) -> Option<OwnedSpan> {
    CURRENT.with(|v| {
        v.borrow()
            .as_ref()
            .filter(|_| STAGES.contains(&name))
            .map(|d| OwnedSpan {
                diagnostics: d.clone(),
                name,
                started: Instant::now(),
            })
    })
}
pub fn current_count(name: &'static str, amount: u64) {
    CURRENT.with(|v| {
        if let Some(d) = v.borrow().as_ref() {
            d.count(name, amount);
        }
    });
}
pub struct Span<'a> {
    diagnostics: &'a Diagnostics,
    name: &'static str,
    started: Option<Instant>,
}
impl Drop for Span<'_> {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            self.diagnostics
                .measure(self.name, started.elapsed().as_secs_f64() * 1000.);
        }
    }
}
pub struct Diagnostics {
    path: Option<PathBuf>,
    state: Mutex<State>,
    started: Instant,
    started_at: i64,
    component: &'static str,
}
impl Diagnostics {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            state: Mutex::new(State::default()),
            started: Instant::now(),
            started_at: now(),
            component: "backend",
        }
    }
    pub fn for_bridge(mut self) -> Self {
        self.component = "bridge";
        self
    }
    pub fn measure(&self, name: &'static str, ms: f64) {
        if self.path.is_none() || !STAGES.contains(&name) {
            return;
        }
        let mut state = self.state.lock().unwrap();
        let samples = state.stages.entry(name).or_default();
        let ms = duration(ms);
        samples.calls += 1;
        samples.total += ms;
        samples.maximum = samples.maximum.max(ms);
        if samples.values.len() < 1024 {
            samples.values.push(ms);
        }
    }
    pub fn count(&self, name: &'static str, amount: u64) {
        if self.path.is_none() || !COUNTERS.contains(&name) {
            return;
        }
        let mut state = self.state.lock().unwrap();
        let counter = state.counters.entry(name).or_default();
        *counter = counter.saturating_add(amount);
    }
    pub fn span(&self, name: &'static str) -> Span<'_> {
        Span {
            diagnostics: self,
            name,
            started: (self.path.is_some() && STAGES.contains(&name)).then(Instant::now),
        }
    }
    fn flush_stages(&self) {
        let (stages, counters) = {
            let mut state = self.state.lock().unwrap();
            (
                std::mem::take(&mut state.stages),
                std::mem::take(&mut state.counters),
            )
        };
        if !counters.is_empty() {
            self.emit("counters", json!({"values":counters}));
        }
        for (name, samples) in stages {
            let omitted = samples.calls.saturating_sub(samples.values.len() as u64);
            self.emit("stage", json!({"stage":name,"durationsMs":samples.values,"calls":samples.calls,"totalDurationMs":duration(samples.total),"maximumDurationMs":samples.maximum,"omittedSamples":omitted}));
        }
    }
    fn emit(&self, event: &str, fields: Value) {
        let Some(path) = &self.path else { return };
        let mut state = self.state.lock().unwrap();
        let t = now();
        let result = (|| -> std::io::Result<()> {
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::other("Log directory required"))?;
            crate::client::private_dir(parent)?;
            let base = path
                .file_name()
                .ok_or_else(|| std::io::Error::other("Log file required"))?
                .to_string_lossy();
            if state.last_cleanup == 0 || t - state.last_cleanup >= 3600000 {
                for entry in std::fs::read_dir(parent)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name != base
                        && !(name.starts_with(&format!("{base}."))
                            && name[base.len() + 1..].parse::<u32>().is_ok())
                    {
                        continue;
                    }
                    let info = std::fs::symlink_metadata(entry.path())?;
                    if !info.is_file() {
                        return Err(std::io::Error::other("Unsafe log file"));
                    }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        if info.nlink() != 1 {
                            return Err(std::io::Error::other("Unsafe log link"));
                        }
                    }
                    if info.modified()?.elapsed().unwrap_or_default().as_secs() >= 14 * 86400 {
                        std::fs::remove_file(entry.path())?;
                        if name == base {
                            state.file_started = None
                        }
                    }
                }
                state.last_cleanup = t;
            }
            let line =
                merge(json!({"schema":3,"time":iso(t),"event":event,"runtime":"rust","runtimeVersion":env!("CARGO_PKG_VERSION"),"processStartedAt":iso(self.started_at),"component":self.component}), fields).to_string() + "\n";
            let info = match std::fs::symlink_metadata(path) {
                Ok(info) => {
                    if !info.is_file() {
                        return Err(std::io::Error::other("Unsafe log file"));
                    }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        if info.nlink() != 1 {
                            return Err(std::io::Error::other("Unsafe log link"));
                        }
                    }
                    Some(info)
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            let size = info.map(|i| i.len()).unwrap_or(0);
            if size > 0 && state.file_started.is_none() {
                let f = std::fs::File::open(path)?;
                let mut line = String::new();
                std::io::BufReader::new(f)
                    .take(65536)
                    .read_line(&mut line)?;
                let first: Value = serde_json::from_str(&line)
                    .map_err(|_| std::io::Error::other("Invalid log header"))?;
                state.file_started = first["time"].as_str().and_then(parse_time);
                if state.file_started.is_none() {
                    return Err(std::io::Error::other("Invalid log time"));
                }
            }
            if size > 0
                && (size + line.len() as u64 > 1024 * 1024
                    || t - state.file_started.unwrap_or(t) >= 86400000)
            {
                for i in (1..=13).rev() {
                    let from = if i == 1 {
                        path.clone()
                    } else {
                        PathBuf::from(format!("{}.{previous}", path.display(), previous = i - 1))
                    };
                    let to = PathBuf::from(format!("{}.{i}", path.display()));
                    if i == 13 && to.exists() {
                        std::fs::remove_file(&to)?;
                    }
                    if from.exists() {
                        std::fs::rename(from, to)?;
                    }
                }
                state.file_started = None;
            }
            let mut opts = std::fs::OpenOptions::new();
            opts.create(true).append(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
            }
            let mut file = opts.open(path)?;
            file.write_all(line.as_bytes())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            }
            state.file_started.get_or_insert(t);
            Ok(())
        })();
        if result.is_err() {
            state.dropped += 1;
            if t - state.last_failure >= 60000 {
                state.last_failure = t;
                eprintln!("BlockContext diagnostics unavailable; measurements dropped.");
            }
        }
    }
    pub fn tool(
        &self,
        name: &str,
        ms: f64,
        data: Option<&Value>,
        error: Option<&AppError>,
        section: Option<&str>,
        bytes: Option<usize>,
    ) {
        let null = Value::Null;
        let data = data.unwrap_or(&null);
        let metadata = &data["metadata"];
        let coverage = &data["coverage"];
        let mut reasons = vec![];
        for (reason, message) in [
            (
                "snapshot_changed",
                "Auction pages changed during the scan; pages from another snapshot were omitted.",
            ),
            (
                "page_fetch_failed",
                "One or more auction pages could not be retrieved.",
            ),
            (
                "scan_stopped",
                "The scan stopped before complete coverage could be established.",
            ),
        ] {
            if arr(&metadata["warnings"]).contains(&json!(message)) {
                reasons.push(reason)
            }
        }
        if n(&coverage["unknownVariantListingCount"]).is_some_and(|n| n > 0.) {
            reasons.push("unknown_variants")
        }
        if n(&coverage["invalidListings"]).is_some_and(|n| n > 0.) {
            reasons.push("invalid_listings")
        }
        let count = |v: &Value| n(v).map(|n| n.round().clamp(0., 1e9));
        let mut row = json!({"tool":TOOLS.contains(&name).then_some(name),"section":section.filter(|s|SECTIONS.contains(s)),"durationMs":duration(ms),"outcome":if error.is_some(){"error"}else{"ok"},"errorCode":error.map(|e|safe_code(&e.code)),"retryAfterSeconds":error.and_then(|e|e.retry_after_seconds),"cache":metadata["cache"].as_str().filter(|s|["fresh","miss","stale"].contains(s)),"snapshotAgeSeconds":count(&metadata["ageSeconds"]),"warningCount":arr(&metadata["warnings"]).len(),"responseBytes":bytes,"sectionStatus":at(data,"result.status").as_str().filter(|s|["available","partial","unavailable"].contains(s)),"auctionCoverage":coverage["status"].as_str().filter(|s|["complete","partial"].contains(s)),"auctionReasons":reasons});
        for (src, dest) in [
            ("scannedPages", "scannedPages"),
            ("totalPages", "totalPages"),
            ("scannedAuctions", "scannedAuctions"),
            ("invalidListings", "invalidListings"),
            ("unknownVariantListingCount", "unknownVariantListings"),
            ("retryAttempts", "retryAttempts"),
            ("recoveredPageFetches", "recoveredPageFetches"),
            ("failedPageFetches", "failedPageFetches"),
        ] {
            row[dest] = json!(count(&coverage[src]));
        }
        row["workload"] = json!(if name == "skyblock_client_context"
            || name == "skyblock_live_context"
        {
            "client_snapshot"
        } else {
            match metadata["source"].as_str() {
                Some("synthetic_demo") => "synthetic_demo",
                Some("hypixel") => "api_snapshot",
                _ => "unknown",
            }
        });
        self.emit("tool", row)
    }
    pub fn upstream(
        &self,
        endpoint: &str,
        ms: f64,
        status: Option<u16>,
        error: Option<&AppError>,
        remaining: Option<u64>,
    ) {
        self.emit("upstream",json!({"endpoint":ENDPOINTS.contains(&endpoint).then_some(endpoint),"durationMs":duration(ms),"httpStatus":status,"outcome":if error.is_some(){"error"}else{"ok"},"errorCode":error.map(|e|safe_code(&e.code)),"remainingRequests":remaining}))
    }
    pub fn http(&self, status: u16, ms: f64, active: usize) {
        self.emit(
            "http",
            json!({"httpStatus":status,"durationMs":duration(ms),"activeRequests":active}),
        )
    }
    pub fn lifecycle(&self, state: &str, mode: &str, transport: &str) {
        if state == "stopping" {
            self.flush_stages();
        }
        self.emit("lifecycle",json!({"state":if ["started","stopping"].contains(&state){Some(state)}else{None},"mode":if ["demo","live"].contains(&mode){Some(mode)}else{None},"transport":if ["http","stdio"].contains(&transport){Some(transport)}else{None}}))
    }
    pub fn resource(&self) {
        if self.path.is_none() {
            return;
        }
        self.flush_stages();
        #[cfg(target_os = "linux")]
        let rss: Option<u64> =
            std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|status| {
                    status.lines().find_map(|l| {
                        l.strip_prefix("VmRSS:")
                            .and_then(|s| s.split_whitespace().next())
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(|n| n * 1024)
                    })
                });
        #[cfg(not(target_os = "linux"))]
        let rss: Option<u64> = None;
        #[cfg(unix)]
        let (user, system): (Option<i64>, Option<i64>) = {
            let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
            if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
                let usage = unsafe { usage.assume_init() };
                // libc timeval uses a 32-bit microsecond field on macOS and a
                // 64-bit field on Linux. Convert through a generic boundary.
                fn micros<S: Into<i64>, U: Into<i64>>(seconds: S, fraction: U) -> i64 {
                    seconds.into() * 1000000 + fraction.into()
                }
                (
                    Some(micros(usage.ru_utime.tv_sec, usage.ru_utime.tv_usec)),
                    Some(micros(usage.ru_stime.tv_sec, usage.ru_stime.tv_usec)),
                )
            } else {
                (None, None)
            }
        };
        #[cfg(not(unix))]
        let (user, system): (Option<i64>, Option<i64>) = (None, None);
        #[allow(
            unused_mut,
            reason = "Linux fills optional resource probes; other platforms report unknown"
        )]
        let mut native = json!({"peakRssBytes":null,"threads":null,"openFileDescriptors":null,"readBytes":null,"writeBytes":null,"voluntaryContextSwitches":null,"involuntaryContextSwitches":null});
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for (field, key, multiplier) in [
                    ("VmHWM:", "peakRssBytes", 1024),
                    ("Threads:", "threads", 1),
                    ("voluntary_ctxt_switches:", "voluntaryContextSwitches", 1),
                    (
                        "nonvoluntary_ctxt_switches:",
                        "involuntaryContextSwitches",
                        1,
                    ),
                ] {
                    if let Some(value) = status.lines().find_map(|line| {
                        line.strip_prefix(field)
                            .and_then(|s| s.split_whitespace().next())
                            .and_then(|s| s.parse::<u64>().ok())
                    }) {
                        native[key] = json!(value.saturating_mul(multiplier));
                    }
                }
            }
            if let Ok(io) = std::fs::read_to_string("/proc/self/io") {
                for (field, key) in [("read_bytes:", "readBytes"), ("write_bytes:", "writeBytes")] {
                    native[key] = json!(io.lines().find_map(|line| {
                        line.strip_prefix(field)
                            .and_then(|s| s.trim().parse::<u64>().ok())
                    }));
                }
            }
            if let Ok(files) = std::fs::read_dir("/proc/self/fd") {
                native["openFileDescriptors"] = json!(files.filter_map(|v| v.ok()).count());
            }
        }
        let dropped = self.state.lock().unwrap().dropped;
        self.emit("resource",merge(json!({"uptimeSeconds":self.started.elapsed().as_secs(),"rssBytes":rss,"heapUsedBytes":null,"externalBytes":null,"cpuUserMicroseconds":user,"cpuSystemMicroseconds":system,"droppedMeasurements":dropped,"runtime":"rust"}),native));
    }
}
fn safe_code(code: &str) -> &str {
    if [
        "API_LIVE_POLLING_DISABLED",
        "SNAPSHOT_REFRESH_REQUIRED",
        "INVALID_RESOURCE",
        "INVALID_LIVE_OPTIONS",
        "RESOURCE_UNAVAILABLE",
        "SUPPLEMENT_UNAVAILABLE",
        "AUCTION_DECODE_FAILED",
        "AUCTION_SCAN_TIMEOUT",
        "AUCTION_SNAPSHOT_EXPIRED",
        "INVALID_AUCTION_QUERY",
        "UPSTREAM_TIMEOUT",
        "UPSTREAM_INVALID",
        "INTERNAL_ERROR",
        "RESPONSE_TOO_LARGE",
        "SNAPSHOT_EXPIRED",
        "DEMO_PLAYER_ONLY",
        "INVENTORY_INVALID",
        "INVALID_MARKET_QUERY",
        "MARKET_ITEM_NOT_FOUND",
        "MARKET_SNAPSHOT_EXPIRED",
        "INVALID_PAGE",
        "INVALID_PROFILE",
        "INVALID_SECTION",
        "NO_SKYBLOCK_PROFILES",
        "PROFILE_REQUIRED",
        "PROFILE_AMBIGUOUS",
        "PROFILE_NOT_FOUND",
        "HYPIXEL_AUTH_FAILED",
        "HYPIXEL_KEY_EXPIRED",
        "HYPIXEL_KEY_REQUIRED",
        "INVALID_AUCTION_PAGE",
        "INVALID_CONFIG",
        "INVALID_PLAYER",
        "PLAYER_NOT_FOUND",
        "RATE_LIMITED",
        "UPSTREAM_REJECTED",
        "UPSTREAM_TOO_LARGE",
        "UPSTREAM_UNAVAILABLE",
        "INVALID_SCHEDULE",
        "SCHEDULE_LIMIT",
        "SCHEDULE_IDENTITY_REQUIRED",
        "CACHE_UNAVAILABLE",
    ]
    .contains(&code)
    {
        code
    } else {
        "INTERNAL_ERROR"
    }
}
