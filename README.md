# BlockContext

Read-only Hypixel SkyBlock context for MCP clients. Your MCP client supplies the reasoning; BlockContext supplies normalized game data, timestamps and explicit coverage. Not affiliated with or endorsed by Hypixel.

## Run it where you want

Use **stdio** for a local MCP client, or **Streamable HTTP** on your own PC/server with a private connection. Requires Node.js 24 or later. The implementation uses standard MCP transports and does not require an OpenAI account. ChatGPT's Secure MCP Tunnel is one optional remote connection method.

Every installation belongs to its operator: your infrastructure, your registered Hypixel application, your credentials and your latest-value cache. There is no shared public BlockContext endpoint. A distributed package does not grant access to another operator's server or API key.

See [self-hosting and client configuration](docs/SELF-HOSTING.md) and [Hypixel access setup](docs/HYPIXEL-SETUP.md).

```sh
npm ci
npm run build
npm run start:demo
```

This starts a **synthetic demo** at `http://127.0.0.1:3008/mcp`; it does not contact Hypixel. Copy `.env.example` to your own private `.env`, set live mode and your own key, then use `npm start`. Never paste the key into a chat, client URL or public repository. Follow Hypixel's application policies for ongoing access; development keys expire.

## Tools

- `skyblock_profile_context`: compact overview, effective skills/caps, currencies, gear, magical power and Garden coverage.
- `skyblock_profile_section`: detailed stats, skills, currencies, slayers, dungeons, pets, collections, minions, equipment, accessories or Garden. Equipment includes backpacks labelled by container. Garden includes player-specific discovered/analyzed Greenhouse mutations.
- `skyblock_market_quote`: Bazaar instant-buy/sell depth for an exact item ID and quantity, plus official NPC sell value.
- `skyblock_auction_quote`: active BIN asking prices for an exact item ID, optionally filtered by rarity, reforge, stars or pet type. Checks page coverage before returning a global lowest price.

Lists use `offset`/`limit` (default 50, maximum 100). Inspect per-list pagination, including nested Greenhouse mutation lists. All tools are read-only. No trades, gameplay actions, coin spending or player-history tracking.

## Data boundaries

Profile/Garden snapshots refresh only on request after five minutes. Transient failures can return an explicitly stale latest snapshot for at most 30 minutes. Authentication failures do not silently return stale profiles. Player-specific Greenhouse data and shared Garden data have separate snapshots; layout, live water, growth and harvest timing remain unknown.

Missing API data is unknown, not zero. Backpack contents are decoded within byte, slot, memory and time limits. Icons are not contents. A damaged container leaves partial coverage and an unknown total. Raw NBT and arbitrary lore are not returned. Co-op member records and bank transaction histories are discarded. SQLite retains one latest normalized value per cache key, with expiry and a 512-key limit; no background refresh runs.

Magical power is a bag-only snapshot calculation outside Dungeons, using pinned upgrade/alias rules, effective rarity and available bonuses. Unknown rules or required fields reduce calculation coverage. Historical highest power is separate. Farming/Taming/Runecrafting caps use available member/rank data. Foraging gates can leave the personal cap unknown. Crafted minions are unlock history, not currently placed minions.

Bazaar quotes walk the visible top 30 order levels; incomplete depth cannot provide a complete fill price. They refresh after 45 seconds and expire after 180 seconds. Auction scans refresh after 60 seconds, expire after 180 seconds from the upstream timestamp, and are bounded to 128 pages, two concurrent reads and 45 seconds. A changed snapshot or missing/corrupt page leaves only observed prices; no global minimum is claimed. Cached listings past their end time are removed from quotes. Active asking prices are not completed sales or an owned-item valuation: enchantments, attributes, skins, pet levels and other variants may differ. Gross virtual coin values exclude fees.

The HTTP listener is loopback-only and checks Host/Origin, request sizes and concurrency. Use a private tunnel, SSH forwarding or an authenticated MCP gateway for remote access. Do not expose the unauthenticated backend directly.

## Validate

```sh
npm run check
npm run smoke
```

The smoke check requires a running HTTP server and accepts `PORT`, `BLOCKCONTEXT_TEST_PLAYER` and `BLOCKCONTEXT_TEST_PROFILE`. A health check alone does not validate Hypixel access. Automated tests use synthetic fixtures and no real key; they exercise both standard transports, parsing bounds, privacy, cache freshness, profile selection, failures and market calculations.

## Sources

Live profiles, skills/items, Garden, Bazaar and auctions come directly from [Hypixel's API](https://api.hypixel.net/). Follow the [API policies](https://developer.hypixel.net/policies/). Incremental Garden thresholds and accessory rules are pinned from the MIT-licensed [NotEnoughUpdates data repository](https://github.com/NotEnoughUpdates/NotEnoughUpdates-REPO/tree/159bb4e7ccc3250780e5daa7525eeb7f13be67e4), with source metadata and its original license in `fixtures/domain`. No remote code or automatic rule updater is used.
