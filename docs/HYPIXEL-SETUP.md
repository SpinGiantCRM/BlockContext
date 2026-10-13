# Configure your own Hypixel access

Sign in to [Hypixel's Developer Dashboard](https://developer.hypixel.net/) and follow its [current API policies](https://developer.hypixel.net/policies/). A development key is suitable for initial tests. For ongoing individual, non-monetized operation, register a Personal application; review/approval is separate from submission. Public non-monetized projects may use Personal applications under the current policy. Monetization, team management or requesting increased limits requires the appropriate Production application.

Describe your deployment accurately: a read-only SkyBlock companion for an MCP client; profile/Garden requests made on demand with caching, latest-only snapshots and freshness/coverage warnings. No gameplay actions, session tracking, player histories, background player polling or general API proxy. Not affiliated with Hypixel. Use your own truthful URL/repository details. Each self-hosted installation registers and uses its own access; credentials from another instance are not included.

Copy `.env.example` into your own private `.env` and set:

```dotenv
BLOCKCONTEXT_MODE=live
HYPIXEL_API_KEY=your_own_key
HYPIXEL_REQUESTS_PER_5_MINUTES=your_assigned_limit
HYPIXEL_API_KEY_EXPIRES_AT=
```

For an expiring development key, set the last field to its real expiry in ISO format with an explicit timezone, for example `2026-10-13T12:45:46+10:30`. Do not invent an expiry or remove a known guard before replacing the temporary credential. The default local request ceiling is a conservative 30 per five minutes; set your assigned quota. Rate-limit headers/reserves and throttling remain enforced.

Restrict the file to your own account (`600` on Unix). Do not paste keys into chat, MCP URLs, logs, public client configuration, a repository or another operator's service. Public resource/market requests do not send the key. See [self-hosting](SELF-HOSTING.md) for stdio and HTTP configurations.

For an HTTP instance, run `blockcontext --env-file /absolute/private/path/.env` and request your intended profile through your MCP client. A result must say `metadata.source: hypixel`; synthetic demo or a successful health check alone does not validate access.
