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
