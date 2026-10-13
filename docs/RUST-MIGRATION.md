# Rust runtime migration

BlockContext 0.11.0 moves the MCP server, Hypixel client, market scans, game normalization, client snapshot reader, SQLite access and SSH bridge to Rust. The Fabric collector remains Java. SQLite remains SQLite: table names, stored latest snapshots and deferred schedules use the existing format. Setup and logging-report scripts remain shell/Python.

The runtime uses the official MCP Rust SDK with stdio and stateless Streamable HTTP. Eight tools retain their names and input contracts. HTTP remains loopback-only; remote clients use their own authenticated gateway or private tunnel. The package contains no shared service, operator credentials or stored player goals/preferences.

## Build and verify

Install Rust 1.88 or newer, then run:

```sh
cargo build --locked --release
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
./target/release/blockcontext --mode demo --transport http --db /absolute/private/path/demo.sqlite
```

On Windows the executable is `blockcontext.exe`. Source builds target Linux, macOS and Windows; CI checks all three. The supplied Linux x86_64 executable requires glibc 2.39 or newer. Build from source on older distributions and other architectures. The optional SSH importer targets Linux and a deliberately configured service account.

Configure an account-only environment file using `.env.example`. Use absolute paths for the environment, database and optional client snapshot. The backend loads a bounded private file before starting its worker threads. Example stdio configuration is in [self-hosting](SELF-HOSTING.md).

## Compatibility evidence

The offline comparison corpus includes 267 comparisons against the previous implementation, covering progression, privacy projection, item decoding, factual contracts and normalized synthetic profiles. Boundary tests cover coalescing and stale caches, blocked API live polling, SQLite schedule persistence, client freshness and ownership, JSON numeric inputs, HTTP limits, upstream expiry/reserves/throttling and stable evidence pagination. Official Rust MCP clients exercise HTTP and stdio; cross-runtime validation also uses a Node MCP client.

Operator validation additionally compared two frozen real API profiles and seven saved collector views with the previous implementation. Those private fixtures are excluded from source and release archives. Reusing the frozen inputs made these comparisons without repeated Hypixel requests.

The migration preserves conservative commission coverage. A saved ledger is not proof of current positive daily bonuses. Exact resets and positive remaining counts require appropriate game evidence; a reference clock alone does not establish them. A real claim/restart test of collector 0.3.6 remains separate from backend compatibility tests.

## Upgrade and rollback

Stop the server before backing up its existing SQLite database, private configuration and service unit. Replace the runtime executable and service command, preserving the database and client snapshot paths. For the optional bridge, use `blockcontext bridge --config /absolute/private/path/client-bridge.json`. A root-owned importer wrapper must run `blockcontext import-snapshot` without forwarding arguments.

Retain the previous executable/service configuration and the pre-upgrade database backup privately until the new instance is verified. No database conversion is required. Configuration of deferred checks causes no API requests; due checks attach only to the next explicit profile snapshot, with no background poller.
