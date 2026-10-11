# Factual game data coverage

Reviewed 11 October 2026. BlockContext 0.8.0 adds nine factual sections and Collector 0.2.0. The desired-data document is an ideal interface, not proof that every requested value is obtainable. Existing capabilities remain available. This release does not claim complete coverage of SkyBlock.

## Scope

Included: observed game progression, inventory and upgrades, currencies, market snapshots, reported activity/timer evidence, and data coverage. No goal plans, player preference records, user-reported facts, recommendation/ranking engine, advice history, money-method comparisons, personal reserves or persistent progression histories. No live owner UUID, private deployment details or player goals are embedded in collector/backend configuration. Demo fixtures remain explicitly synthetic. API snapshots and latest client observations have bounded expiry and are not a memory database.

## Sources and request load

The API remains the source for a broad profile baseline. Authenticated player snapshots refresh only on a user request, after five minutes; missing data is not continuously polled. The passive collector reads current owner inventory and supported displayed UI. Its latest snapshot exports every five seconds. The separate optional bridge syncs changed snapshots to the operator's own server. `skyblock_client_context` makes zero API requests and supports the same section names as `skyblock_profile_section`; it returns partial observations, not a fabricated complete profile. Frequent client reads are the route for fresher visible state. Standard profile tools return API data with independent `clientObservations`; conflicting sources are preserved instead of silently overwritten.

## Current coverage

| System | Factual API support | Client support and limits |
| --- | --- | --- |
| Mining | Modern/legacy HotM XP, calculated tier/progress, recorded node levels/toggles, preset/ability, spent tokens, separately reported powder counters, forge jobs and crystal/core evidence | Selected numeric fields in manually opened HotM, commissions, forge and Nucleus menus. Node definitions, unlock costs, available powder semantics, commission objectives/reset rules and forge duration modifiers are not fully verified. |
| Foraging | HotF XP/tier/progress, nodes, whispers and API core/progression evidence | Selected numeric facts in HotF/Whispers menus. Full current perk definitions and gifts/reset eligibility remain unknown. |
| Hunting | Owned shard IDs/counts, trap/fusion evidence, reported attribute stacks and toolkit evidence | Current Hunting tools, Huntaxe stored-weapon tooltip name/stars, optional full weapon item in a recognized Huntaxe slot, typed Hunting menu fields. Stacks do not establish effective attribute levels. |
| Huntaxe weapon | Stored-weapon name and displayed stars can appear in the Huntaxe tool's own API item lore. Reports from multiple tools can conflict. Full absorbed-weapon enchantments and upgrades are not present in that summary. | Current tool lore is captured without opening a menu. A full slot-22 weapon item requires manually opening the recognized Huntaxe menu. Missing/loading slots never prove an empty Huntaxe. |
| Items/storage | Existing carried/equipped gear, wardrobe sets, vault, Ender Chest, backpack and sack coverage; added hashed instance IDs, scrolls, gems, attributes/runes and selected upgrade/use/fuel counters | Current inventory slots 0–40, recognized manually viewed storage/toolkit menus, selected upgrades/scrolls/gems. Unviewed pages, placed blocks, Museum holdings and all embedded items are not a complete scan. Identical menu titles can replace prior observations. |
| Currency/buffs | Separate personal/shared bank fields, purse, reported currencies/essence, bits/fame evidence, copper where reported; existing Cookie flag and effect lists | Explicitly labeled bank balances, separately unclassified bank balances, displayed Cookie/God Potion timers and typed tab facts. Unknown bank fields remain unknown. No net-worth, escrow or spending-budget estimate. |
| Dungeons | Level/class XP, per-floor completion counts including API total entries, reported performance maps/best runs, API recent-run party size and own milestone, chest rewards/paid flag | Current carried gear is visible; it does not certify an active dungeon loadout. Other participants' identities are removed. Chest costs/profit, carry dependence and current complete entry rules remain unverified. No history collector. |
| Dailies | One view of reported experimentation, mining, forge, foraging, dungeon, Garden, Crimson, chocolate and shared upgrade evidence | Selected typed facts from manually opened experiments and relevant menus. A saved claim flag or charge timestamp is not proof of completion today. Eligibility, charges, resets and rewards remain unknown unless explicitly observed/verified. |
| Quests/Guide/travel | Objective ID, reported state/progress, nonzero completion timestamp, reported quests, visited areas, unlocked collection tiers, SkyBlock Guide/task evidence and shared upgrade evidence | Selected numeric facts from supported Guide/Quest/upgrade menus. Full shop inventories and reward eligibility are not modelled. |
| Rift/Crimson/chocolate/bestiary | Bounded reported progression evidence with timestamps/source; chocolate balance; reported combat counters | No full live formula, prestige simulation, effective combat stat model or event-calendar engine. Undocumented evidence is labelled partial. |
| Existing skills/pets/accessories/Garden/markets | Skill caps, pet levels, autopet/pet-system evidence, collections, accessory diagnostics/MP and Garden/Greenhouse progression, Bazaar order depth, bounded auction variant snapshots | Selected typed facts from supported pet/collection/accessory/stat menus and previously supported Garden menus. No whole-island minion setup, measured profit/hour or simulated live Greenhouse. |

Menu recognition and numeric parsing are conservative. They accept known titles/labels and fail closed when formats differ. A menu's numeric fields have display context, slot, value, maximum/unit where printed, and a separate observation timestamp. They are not a complete semantic model of the system. Tests use controlled menu examples; real menu coverage needs validation on the intended client.

Lists have pagination or bounded projection with coverage/truncation metadata. Missing fields stay unknown, known zero is retained, and zero completion timestamps are not actual dates. Hashed item instance identifiers allow matching without exposing raw item UUIDs. Game-controlled strings remain untrusted data.

## Remaining evidence gaps

Full current node definitions/effects/prerequisites, reliable daily reset rules, effective attributes/stats by location, complete minion/island/Museum contents, orders/escrow, exact gear valuation, full absorbed-weapon data when the menu is not readable, and complete active dungeon loadouts still need verified API/UI sources. Read `systems`, per-field coverage and each section's warnings before relying on completeness. Future fields are added through tested projections, not arbitrary raw NBT/lore dumps or client automation.

## Policy and data references

- [Hypixel API policies](https://developer.hypixel.net/policies/) reviewed 11 October 2026: cache requests, keep keys out of public mods, and do not continuously API-poll players for stats/session history.
- [Hypixel allowed modifications](https://support.hypixel.net/hc/en-us/articles/6472550754962-Hypixel-Allowed-Modifications): our collector is limited to recording owner-visible state, without gameplay input or protocol changes. This is a design assessment, not Hypixel mod approval or a guarantee.
- [Pinned NEU HotM/HotF XP tables](https://github.com/NotEnoughUpdates/NotEnoughUpdates-REPO/blob/79c0356d2117e40f1d75c06861126ebaa38c5af6/constants/leveling.json), MIT notice bundled. Calculations identify the reference revision; live current game rules are not asserted verified.
