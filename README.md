# BlockContext

Read-only Hypixel SkyBlock context for MCP clients. Your MCP client supplies the reasoning; BlockContext supplies normalized game data, timestamps and explicit coverage. Not affiliated with or endorsed by Hypixel.

## Run it where you want

Use **stdio** for a local MCP client, or **Streamable HTTP** on your own PC/server with a private connection. Requires Node.js 24 or later. The implementation uses standard MCP transports and does not require an OpenAI account. ChatGPT's Secure MCP Tunnel is one optional remote connection method.

Every installation belongs to its operator: your infrastructure, your registered Hypixel application, your credentials and your latest-value cache. There is no shared public BlockContext endpoint. A distributed package does not grant access to another operator's server or API key.

See [self-hosting and client configuration](docs/SELF-HOSTING.md), [Hypixel access setup](docs/HYPIXEL-SETUP.md), and [optional local logging](docs/LOGGING.md).

```sh
npm ci
npm run build
npm run start:demo
```

This starts a **synthetic demo** at `http://127.0.0.1:3008/mcp`; it does not contact Hypixel. Copy `.env.example` to your own private `.env`, set live mode and your own key, then use `npm start`. Never paste the key into a chat, client URL or public repository. Follow Hypixel's application policies for ongoing access; development keys expire.

See the optional [passive client collector](docs/CLIENT-COLLECTOR.md) for data you manually view in game that the API does not expose. It runs locally without an API key; a separate bridge can sync to your own server.

## Tools

- `skyblock_profile_context`: compact overview, effective skills/caps, currencies, gear, magical power and Garden coverage.
- `skyblock_profile_section`: detailed stats, skills, currencies, slayers, dungeons, pets, collections, minions, equipment, accessories or Garden. Equipment includes backpacks labelled by container and modern armor/equipment wardrobe sets. Currencies distinguish personal from shared bank balances; fieldCoverage explains missing or unreadable data. Pets have calculated levels and progress to their species cap (including level-200 dragons); Dungeons include effective Catacombs/class levels and the next normal-floor level gate. Garden includes player-specific discovered/analyzed Greenhouse mutations.
- `skyblock_client_context`: latest owner-only client observations of viewed Huntaxe/storage menus, displayed balances and cookie/God Potion timers, with independent timestamps and partial coverage. Works without a Hypixel API request when configured.
- `skyblock_market_quote`: Bazaar instant-buy/sell depth for an exact item ID and quantity, plus official NPC sell value.
- `skyblock_auction_quote`: active BIN asking prices for an exact item ID, optionally filtered by rarity, reforge, stars, pet type, exact pet level or exact ability scroll set. Listings include pet rarity/XP/level/held item/skin and Wither blade scrolls. Checks page and filtered-variant coverage before returning a global lowest price.

Lists use `offset`/`limit` (default 50, maximum 100). Inspect per-list pagination, including nested Greenhouse mutation lists. All tools are read-only. No trades, gameplay actions, coin spending or player-history tracking.

## Data boundaries

Profile/Garden snapshots refresh only on request after five minutes. Transient failures can return an explicitly stale latest snapshot for at most 30 minutes. Authentication failures do not silently return stale profiles. Player-specific Greenhouse data and shared Garden data have separate snapshots; layout, live water, growth and harvest timing remain unknown.

Missing API data is unknown, not zero. Backpack contents are decoded within byte, slot, memory and time limits. Icons are not contents. A damaged container leaves partial coverage and an unknown total. Raw NBT and arbitrary lore are not returned. Co-op member records and bank transaction histories are discarded. SQLite retains one latest normalized value per cache key, with expiry and a 512-key limit; no background refresh runs.

Magical power is a bag-only snapshot calculation outside Dungeons, using pinned upgrade/alias rules, effective rarity and available bonuses. Unknown rules or required fields reduce calculation coverage. Historical highest power is separate. Farming/Taming/Runecrafting caps use available member/rank data. Foraging gates can leave the personal cap unknown. Crafted minions are unlock history, not currently placed minions.

Bazaar quotes walk the visible top 30 order levels; incomplete depth cannot provide a complete fill price. They refresh after 45 seconds and expire after 180 seconds. Auction scans refresh after 60 seconds, expire after 180 seconds from the upstream timestamp, and are bounded to 128 pages, two concurrent reads and 45 seconds. A changed snapshot or missing/corrupt page leaves only observed prices; no global minimum is claimed. Cached listings past their end time are removed from quotes. Active asking prices are not completed sales or an owned-item valuation: enchantments, attributes, skins and other variants may differ. Fully scrolled and level-200 filters do not certify all other upgrades as maxed. Unknown variant details prevent a global minimum for that filtered quote. Gross virtual coin values exclude fees.

The HTTP listener is loopback-only and checks Host/Origin, request sizes and concurrency. Use a private tunnel, SSH forwarding or an authenticated MCP gateway for remote access. Do not expose the unauthenticated backend directly.

## Validate

```sh
npm run check
npm run smoke
```

The smoke check requires a running HTTP server and accepts `PORT`, `BLOCKCONTEXT_TEST_PLAYER` and `BLOCKCONTEXT_TEST_PROFILE`. A health check alone does not validate Hypixel access. Automated tests use synthetic fixtures and no real key; they exercise both standard transports, parsing bounds, privacy, cache freshness, profile selection, failures and market calculations.

## Sources

Live profiles, skills/items, Garden, Bazaar and auctions come directly from [Hypixel's API](https://api.hypixel.net/). Follow the [API policies](https://developer.hypixel.net/policies/). Incremental Garden thresholds, accessory rules, and pet/Catacombs XP tables are pinned from the MIT-licensed [NotEnoughUpdates data repository](https://github.com/NotEnoughUpdates/NotEnoughUpdates-REPO/tree/159bb4e7ccc3250780e5daa7525eeb7f13be67e4), with source metadata and its original license in `fixtures/domain`. No remote code or automatic rule updater is used.

## Comparing variants

Use `skyblock_auction_quote` with `{"item":"HYPERION","scrolls":["IMPLOSION_SCROLL","SHADOW_WARP_SCROLL","WITHER_SHIELD_SCROLL"]}` for the exact three-scroll set, or `{"item":"PET","petType":"GOLDEN_DRAGON","petLevel":200}` for level-200 Golden Dragons. An empty `scrolls` array selects unscrolled blades; omit the filter to include all scroll sets. Integer pet levels use the petInfo rarity XP curve, which can differ from a boosted display rarity.

Pet and dungeon conversions cite a pinned community data revision. Catacombs levels stop at 50 and show overflow XP separately. The next unlock describes the next normal-floor level requirement from a cited patch; it does not certify Combat level, previous-floor completion or later rule changes. Modern wardrobe items preserve set and piece identifiers. Missing backpacks or bank fields remain unknown when Hypixel does not expose them.


## Cookie and potion snapshots

The stats section includes API-reported Booster Cookie active/inactive status, active and paused potion-effect lists, safe effect identifiers, levels and reported duration ticks when present. `false` and known empty lists differ from missing data. Lists are bounded to 256 entries each and paginated in detailed stats; the overview shows at most ten of each. Field coverage identifies missing or malformed entries. Dedicated God Potion status and cookie/God Potion timers remain unknown; individual effects do not establish a God Potion. Saved tick durations are not live countdowns, and no wall-clock expiry is inferred. These fields use the existing profile request and its cache timestamps.

Huntaxe tools can expose a stored-weapon display name and stars in API item lore. `huntaxeStoredWeapon` labels this partial evidence; full enchantments/upgrades remain unknown, and conflicting tools stay unresolved. The optional collector can record the full displayed weapon item from a manually opened supported menu. See [factual data coverage](docs/FACTUAL-DATA-COVERAGE.md) for the expanded mining, foraging, Hunting, dailies, quests and other system sections, client overlap and remaining gaps. BlockContext stores no user goals, preferences or recommendation history.
