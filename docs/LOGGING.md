# Optional local operational logging

Set `BLOCKCONTEXT_LOG_PATH` to a private local file to enable logging. Logging is off by default and never sends telemetry to another operator or an external service.

Records contain tool name, allowlisted section, success/error code, duration, cache state, snapshot age, response size, warning count, section availability and auction scan coverage. Auction reasons distinguish snapshot changes, unreadable listings and unknown filtered variants. Upstream measurements contain endpoint category, status, duration and remaining request budget. HTTP records contain status, duration and active request count. Resource snapshots once per minute contain uptime, memory and cumulative CPU usage. Lifecycle events record startup and shutdown.

No request parameters, player names or UUIDs, profile identities, item queries, tool response bodies, prices, inventory contents, URLs, IPs, headers, tokens, API keys, arbitrary warnings or exception messages are recorded. No user/session identifiers or gameplay polling are introduced.

Use a private directory owned by the service account (0700). Files are account-only (0600), and MCP/HTTP never exposes them. Daily rotation or a 1 MiB size limit, with 13 archives, bounds storage to about 14 MiB and retention to approximately 14 days (heavy traffic shortens retention). Expired files are removed during logging. For a hardened systemd deployment, add only this directory to `ReadWritePaths`. Logging failures do not fail MCP requests; a generic, rate-limited stderr message reports dropped measurements.

The optional Python helper `deploy/logging-report.py` summarizes the local files:

```sh
python3 deploy/logging-report.py --directory /var/log/blockcontext --hours 24
```

It reports p50/p95/worst durations, cache use, error/throttle counts, coverage and memory. Resource sampling can miss a brief peak; service memory peaks remain available through systemd. No automatic reports or external telemetry uploads are configured.

Collection starts when enabled. Installation/validation calls are reflected in counts; subsequent windows reflect normal use. Tests cover privacy, real HTTP/stdio integration, upstream throttling, rotation, expiry and unavailable sinks.

Auction measurements include bounded retry attempts, recovered page fetches and final failed page fetches. The report totals these only for cache-miss responses and excludes cache hits. Concurrent callers sharing a scan can each report the same scan counters. Response-body timeouts use `UPSTREAM_TIMEOUT`; malformed JSON remains `UPSTREAM_INVALID`.

## Native Rust profiling

Schema 2 marks every record with the runtime, build version and process start timestamp. Durations preserve microsecond precision in millisecond units. Stage samples stay in memory and are emitted once per resource interval (60 seconds) and on graceful shutdown, with at most 1024 timing samples per stage per interval. Total calls and omitted samples expose partial sampling; stage events do not perform per-request file writes. A crash can lose the unfinished interval. New bounded stage measurements separate SQLite snapshot reading/parsing, cache serialization/writes, profile normalization, client snapshot reading and MCP payload serialization. Stage spans may overlap; do not add them or subtract unrelated upstream requests from a tool duration.

The Python report remains an offline analysis utility. It reads old and new logs, groups runtime/version results, separates cache misses from fresh/stale reads, handles unavailable counters and never derives CPU deltas across processes or restarts. Legacy rows without version metadata are labelled unknown. Historical periods contain different traffic; they do not measure a controlled speedup.

```sh
python3 deploy/logging-report.py --directory /var/log/blockcontext --hours 24 --runtime rust
```

RSS and process CPU counters apply to Rust. Node heap/external-memory figures do not describe Rust allocations and remain absent. Linux supplies current RSS; unsupported counters remain unknown. The existing bounded rotation, private permissions and no-player-data policy continue. Extra stage records consume the same approximately 14 MiB total log budget, so busy installations may have shorter retention.

## Controlled offline comparisons

`deploy/benchmark-runtimes.py` runs the operator's retained Node backend and Rust binary on one host, in counterbalanced order, with fresh private demo caches and logging both disabled and enabled. It refuses a live-mode health response and requires synthetic source markers. It makes no upstream requests. Supply absolute paths to the previous Node runtime/source and current Rust binary; the baseline is not included in the Rust release.

```sh
python3 deploy/benchmark-runtimes.py --node /absolute/path/to/node --node-main /absolute/path/to/previous/dist/src/main.js --rust /absolute/path/to/blockcontext --rounds 4 --output /absolute/private/path/benchmark.json
```

The synthetic workload measures first-snapshot latency, cached overview/equipment latency distributions, sampled process CPU, startup and Linux peak RSS. It does not model a full real inventory, auction scan, Internet latency or ChatGPT reasoning. Low-duration samples and VM scheduling can vary; compare repeated runs and CPU alongside latency. It is not a Hypixel API load test.

For deeper stack profiling, build the optional `profiling` Cargo profile with symbols and frame pointers:

```sh
RUSTFLAGS="-C force-frame-pointers=yes" cargo build --locked --profile profiling
```

Use your platform's sampling profiler on an isolated **demo-mode** process with a separate private cache. On Linux, `perf record -g -- ./target/profiling/blockcontext --mode demo --transport http --db :memory:` records samples while a local MCP client makes synthetic requests; stop it after the bounded investigation. Keep profiler outputs private. Production binaries remain stripped; stack/allocation sampling is not enabled in production. No Rust allocator-specific allocation profiler is installed or implied by RSS measurements.
