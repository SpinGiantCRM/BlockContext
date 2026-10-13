# Deferred checks on requested profile snapshots

`skyblock_poll_schedule` stores bounded operational checks in the operator's private SQLite database. It never creates a timer, agent task or background API request. Each check belongs to one player UUID and exact profile name. There are at most 16 checks per player/profile and 256 per installation, with one latest schedule state and no run history.

## Configure a check

```json
{
  "player": "YOUR_PLAYER_UUID",
  "profile": "YOUR_PROFILE_NAME",
  "operation": "set",
  "id": "hotm-reset",
  "target": "hotm_bonus_commissions",
  "cadence": {"kind": "hotm_reset_reference"}
}
```

The HotM preset uses the expected fixed 05:00 UTC reset from [NEU's implementation](https://github.com/NotEnoughUpdates/NotEnoughUpdates/blob/9b1fcfebc646e9fb69f99006327faa3e734e5f51/src/main/java/io/github/moulberry/notenoughupdates/overlays/TimersOverlay.java). This is reference metadata, not a verified server timestamp. The [original official Dwarven Mines patch](https://hypixel.net/threads/skyblock-patch-0-11-dwarven-mines.3749492/) describes four daily bonuses; the reference does not prove four are left now. The collector records replenishment observation windows so the reference can be compared with actual received game UI. Exact reset timestamps stay unknown unless explicitly exposed.

Other targets are `currencies`, `stats`, `skills`, `mining` and `dailies`. These reuse fields from the requested profile endpoint. Garden, Museum, orders and other supplemental endpoints are deliberately outside this scheduler.

Use `{"kind":"daily_utc","timeUtc":"05:00"}` for a daily UTC check or `{"kind":"once","at":"2026-10-13T05:00:00Z"}` for a one-time check. Daily schedules skip missed intervals and produce one current evaluation on the next request; they do not backfill history. UTC schedules do not follow the host timezone or daylight saving.

Use `operation: "list"` to inspect checks, or `operation: "remove"` with an `id` to remove one. Supply a player UUID, or a username already known to the snapshot cache or owner collector. Configuration performs zero API requests and never contacts an identity service. Setting a check replaces that ID and restarts its schedule. These are operational settings, not player goals, preferences or recommendation history.

## Consume a due check

When an agent next calls `skyblock_profile_context` or `skyblock_profile_section` with **`mode: "snapshot"`**, due checks for the selected profile are included in top-level `scheduledChecks`. For example, a requested currencies snapshot can also include the due HotM check. API-section checks require a post-due profile snapshot and coalesce into that same profile request; they never add another endpoint call. Failed/stale snapshots do not consume them. Cached reads, live collector reads, market tools and schedule configuration never trigger or consume deferred checks.

A HotM check reads client evidence separately from the API. If the current remaining count is unknown, its result explains the gap and remains `pending: true` for the next explicitly requested snapshot. A pending client-only check never forces an extra profile refresh. A verified post-due displayed count lets the check advance. A reference reset crossing alone does not establish that the server reset occurred or that four bonuses remain.

Claim totals persist across Minecraft restarts in a private per-player/profile ledger. Exact positive remaining counts require an explicit displayed starting counter and continuously observed bonus claims; gaps invalidate that coverage. The currently verified tooltip exposes bonus availability or exhaustion, but no positive remaining counter. Therefore this version cannot guarantee an exact positive count for every session. It reports that limitation rather than deriving a purported exact count from the clock. The API's daily ore counters are not commission claim counters.

This design retains caching, quota handling and the prohibition on API live polling. See [Hypixel's API policy](https://developer.hypixel.net/policies/). The passive collector sends no server requests and automates no gameplay. It contains no Hypixel key.
