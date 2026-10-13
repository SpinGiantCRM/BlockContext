# Third-party notices

BlockContext source uses the MIT license in the root LICENSE file. Hypixel's API policies remain applicable to API usage, regardless of the source license.

Pinned Garden/accessory/pet, Catacombs and HotM/HotF XP data is derived from NotEnoughUpdates-REPO under MIT. Its original notice is included at `fixtures/domain/NEU-LICENSE.txt`; the bundled subsets retain source revision metadata. No SkyCrypt source code or constants table is included.

Rust dependencies are pinned in Cargo.lock. They include the official MCP Rust SDK (rmcp), Axum, Tokio, Reqwest, Rusqlite, Serde, FastNBT and Flate2. SQLite is bundled through Rusqlite. Dependency license metadata is included in native release archives; review upstream notices when redistributing. The collector uses Minecraft/Fabric separately and does not bundle the game.

Normal-floor level gates retain the [30 March 2021 staff patch](https://hypixel.net/threads/skyblock-0-11-3.4058260/) as a historical reference. A source review on 11 October 2026 found no newer accessible authoritative confirmation of all current gates; the official wiki closed in July 2026. Responses expose `floorRequirementReview` separately from the original publication date and mark current rules unverified. Recheck this reference when updating progression data; do not treat the next level gate as permission to enter a floor.

The pinned NEU revision `79c0356d2117e40f1d75c06861126ebaa38c5af6` also supplies HotM/HotF node layout/expressions, shard metadata/thresholds and bestiary definitions in `systems-definitions.json`. Its MIT license is bundled as `fixtures/domain/NEU-LICENSE.txt`. SkyCrypt Backend revision `5252bf610310893598916301753486bc4b13ab7d` was consulted to verify powder/whisper counter semantics and numerical token/Slayer thresholds; no SkyCrypt source code is bundled. These are reference facts, not an assertion of verified current game formulas.
