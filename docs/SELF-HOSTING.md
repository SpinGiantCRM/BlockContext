# Self-host BlockContext with an MCP client

BlockContext supports standard **stdio** and **Streamable HTTP**, independently of any one client vendor. Each operator runs their own instance with their own registered Hypixel access. The maintainer's private server, key and tunnel are not part of the package.

## Requirements and installation

Use Node.js 24 or later on Linux, macOS or Windows. The repository CI exercises both MCP transports and the feature tests on Linux, macOS and Windows. See the [portability results](https://github.com/SpinGiantCRM/BlockContext/actions/workflows/ci.yml). The stdio launcher and file paths use Node APIs without requiring a shell. Serverless/edge runtimes that lack Node workers and persistent SQLite are not supported.

Extract the package into a directory you control. From that directory:

```sh
npm ci
npm run build
```

Runtime archives already contain `dist/src`; source archives also include the tests. Dependencies are pinned in the lockfile. Open `.env.example`, save a private copy as `.env`, and configure your own Hypixel application using [the setup guide](HYPIXEL-SETUP.md). Keep credentials readable only by your account. Use your OS file permissions; never include the private file in source control, an archive or client output. On Unix, restrict credential files to mode 600 and their directory to 700. On Windows, use the folder/file Security settings to restrict the private configuration and cache directory to your account; Unix chmod mode bits do not enforce Windows ACLs.

## Local clients: stdio

Set these entries in your private file:

```dotenv
BLOCKCONTEXT_TRANSPORT=stdio
BLOCKCONTEXT_MODE=live
HYPIXEL_API_KEY=your_own_key
HYPIXEL_REQUESTS_PER_5_MINUTES=your_assigned_limit
BLOCKCONTEXT_DB=/absolute/private/path/cache.sqlite
```

Use absolute paths so the client's working directory does not matter. A common MCP-client configuration shape is:

```json
{
  "mcpServers": {
    "blockcontext": {
      "command": "/absolute/path/to/node",
      "args": [
        "--env-file=/absolute/path/to/BlockContext/.env",
        "/absolute/path/to/BlockContext/dist/src/main.js"
      ]
    }
  }
}
```

Client setting names vary; supply the same command/arguments in its standard stdio-server settings. On Windows, JSON accepts paths such as `C:/BlockContext/.env`. The command must identify the installed Node executable. Only MCP messages go to stdout; diagnostics go to stderr. No HTTP port is opened in stdio mode. The client launches and stops its own process. Demo mode can validate discovery before registering access; its values remain explicitly synthetic.

## HTTP clients: local or your own server

Set `BLOCKCONTEXT_TRANSPORT=http`, choose a local port (default 3008), and run `npm start`. Point a Streamable HTTP client at `http://127.0.0.1:3008/mcp`. The server is stateless at the transport layer; the latest-value cache persists separately.

For another machine, keep the backend on loopback and connect over verified SSH forwarding, a private authenticated tunnel, or an authenticated MCP gateway. For example, forward a local port to your own server's `127.0.0.1:3008` and use that local URL. Do not bypass SSH host-key validation. The included system-service templates can be adapted to your own paths/account/runtime; use separate private environment and cache files.

For a public HTTPS URL, terminate standards-compliant authentication at a trusted MCP gateway and proxy internally to loopback. Configure the gateway's own Host/Origin and audience checks; the backend's loopback Host/Origin guards must remain intact. This package does not implement OAuth or provide a shared hosting service. A URL alone is insufficient security.

ChatGPT can optionally use [OpenAI Secure MCP Tunnels](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels), where available to the operator's account. Create your own tunnel/workspace association and your own scoped runtime key. Other MCP clients do not need that tunnel. Never reuse someone else's tunnel or credentials.

## Verify the result

Initialize the client and discover seven read-only tools. Request a profile overview, then equipment and Garden detail. Verify source `hypixel`, the intended profile, timestamps and coverage. A missing inventory or mutation list is unknown; a demo result does not validate a live key. Request Bazaar and auction quotes separately. Auction initialization/discovery does not trigger a scan; the first quote may take up to 45 seconds. Configure the client tool timeout to at least 60 seconds.

Use `npm run check` to run the source archive's offline tests and dependency audit. In HTTP mode, `npm run smoke` checks the actual transport against a running server. It prints normalized player data, so keep its output private.

## Optional client data

The [client collector guide](CLIENT-COLLECTOR.md) covers the supported Fabric version, passive recording boundaries, local stdio integration and optional SSH syncing to your own server. Configure `BLOCKCONTEXT_CLIENT_SNAPSHOT_PATH` only for a private owner snapshot. `skyblock_client_context` does not depend on live Hypixel access; unknown observations remain unknown.

## Maintenance

Rotate or renew your own expiring credentials privately and restart your instance. Set a known development-key expiry with an explicit timezone. Update pinned domain rules only after reviewing and testing them. Back up configuration and source separately from player-cache files. Do not add background player polling or histories. No monetization is built in; check Hypixel's policy before changing the use model.

For live polling, configure the optional collector and use `skyblock_live_context`, with no API fallback. Profile tools default to cached mode and reject live mode. Use explicit snapshot mode for an occasional intentional API check, never a polling loop. Public resources/markets have separate caching and rate limits. See [factual coverage](FACTUAL-DATA-COVERAGE.md) for all section names and limits.
