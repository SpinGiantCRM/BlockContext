# Optimisation runbook

Start with an offline report from the existing private logs. Reading these files never polls Hypixel, the game or an MCP tool.

## Collect a baseline

On a service installation:

```sh
sudo /usr/local/bin/blockcontext-report --directory /var/log/blockcontext --hours 24 --runtime rust --component backend
systemctl show blockcontext.service -p MemoryCurrent -p MemoryPeak -p CPUUsageNSec -p NRestarts
```

For other installations, run `python3 deploy/logging-report.py` against your private log directory. Compare 1-hour and 24-hour windows; keep runtimes, versions, components, cache states and synthetic/API/client workloads separate. A few samples or omitted timing samples cannot establish a reliable p99. CPU intervals must belong to the same process. `MemoryPeak` and Linux lifetime peak RSS can reveal spikes that a one-minute sample missed.

## Find the expensive layer

| Evidence | Inspect next |
| --- | --- |
| Slow upstream headers/body, local stages short | Connection reuse, timeouts, remote service latency; avoid extra API polls |
| Cache reads slow, `cache_json_parse` high | Snapshot volume, repeated parsing/copies and bounded caches |
| `cache_lock_wait` or worker/CPU queue waits high | Contention, blocking work and concurrency limits |
| SQLite writes or cache JSON serialization high | Stored snapshot size, transaction volume and eviction work; keep database compatibility |
| Inventory/gzip/NBT stages high | Encoded/expanded byte counts, repeated decoding, safety limits and representative fixtures |
| Field mapping/normalization high | Repeated transforms, cloning and unnecessary traversal |
| Client file reading/validation high | Snapshot volume and repeated validation; preserve owner/freshness checks |
| Payload serialization high | Response size, pagination and repeated serialization |
| Bridge projection/SSH uploads high or failing | Unchanged/upload byte counters, verified SSH connectivity and fixed importer permissions |
| Growing RSS, descriptors, threads or context switches | Bounded concurrency, process lifetime trends and resource retention |
| Resource tick lateness high | Runtime blocking, host scheduling or saturation; not proof of one specific cause |

Counts are operation totals, not unique users. Cache coalescing can serve several requests with one load. Stage durations overlap; do not add them or subtract unrelated network events from a request. Kernel I/O counters and byte-volume counters measure different layers.

## Reproduce without upstream load

Use the controlled synthetic benchmark documented in [LOGGING.md](LOGGING.md). Keep hardware, runtime versions, fixture, transports, logging setting and concurrency fixed; alternate runtime order and record latency distributions plus CPU and memory. Preserve the baseline JSON privately. Results from a small synthetic profile do not establish full-inventory or auction performance.

For a production-sized case, privately save an already-authorized snapshot and compare the same frozen input offline. Do not repeatedly fetch live player data to generate samples. Private fixture files and raw logs must remain outside public source archives.

## Deep profiling when needed

The optional Cargo `profiling` profile retains symbols. Build with frame pointers and use your platform's sampling profiler on an isolated demo process, or a deliberately bounded offline replay. Keep symbolised traces private. No stack or allocation sampling runs in production by default, and RSS is not a count of live Rust allocations.

## Change and verify

Change one bottleneck, rerun the same bounded workload, compare CPU/memory as well as p50/p95/p99, and verify that output contracts and uncertainty labels still match. Run `cargo test --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`, and `python -m unittest discover -s deploy/tests -p 'test_*.py'`. Preserve existing credentials, database and last-value snapshots; retain a previous executable/report for rollback. Logging/collector changes do not authorize gameplay actions or background Hypixel polling.

## Limits and access

Logging is opt-in in the distributed package. Each component keeps account-only logs with a 1 MiB file limit, 13 archives and approximately 14-day maximum retention; busy traffic shortens the time covered. Backend and bridge have independent budgets. Buffered samples are lost if the process crashes before flushing. There is no public log endpoint, raw-request recording or automatic report upload. If the sink fails, measurements are counted as dropped and requests continue.
