# Optional passive client collector

BlockContext's Fabric collector records selected information already displayed in the owner's SkyBlock menus and tab display. It writes one private local snapshot. It contains no Hypixel API key, HTTP client, remote endpoint, chat capture, gameplay commands, clicks or packet modifications. It never opens menus for you. It is not affiliated with or approved by Hypixel.

## API gaps and present coverage

This investigation distinguishes data omitted in an observed profile from data that is never documented or verified in the public API. Bank and backpack contents can be exposed for other profiles; their omission does not prove a disabled setting or an empty account. Known API data remains separate and unchanged.

| Data | Public API finding | Collector coverage |
| --- | --- | --- |
| Absorbed Huntaxe weapon | Tool lore can expose a partial display name/stars; full stored-weapon fields are unverified | Weapon displayed in slot 22, identified as a weapon by its structured item footer; ID, reforge, enchantments and selected upgrade counters |
| Bank balances omitted on a profile | Personal and shared bank fields exist, but can be absent even with sharing enabled | Displayed balances; only explicit personal/co-op labels classify the account. Otherwise the balance stays unclassified |
| Booster Cookie timer | API active flag is supported; no verified remaining-time field | Remaining duration/status displayed in the SkyBlock menu or tab footer |
| God Potion status/timer | Individual effect lists do not establish a God Potion | Explicit God Potion tab/menu display; Active Effects slot 10 only when the God Potion filter is selected |
| Unavailable backpacks/Ender Chest | Contents depend on exposed inventory fields | Contents of recognized menus you manually open, excluding the bottom player-inventory slots; partial coverage |
| Hunting Toolkit | Empty public toolkit data does not establish empty storage | Items displayed in the manually opened toolkit |
| Currently placed minions | Crafted-minion API list describes unlock history | Items in the latest viewed minion menu; no island-wide scan, positions or production-rate estimate |
| Live Greenhouse/Composter | Discovered/analyzed mutations are supported; live layout/water/growth timing is not implemented as verified API data | Items in recognized viewed menus; displayed next-growth-stage duration from Crop Diagnostics slot 20. Crop layout, water, harvest times and a complete live Greenhouse remain unknown |

Menu formats change. Unrecognized, ambiguous, loading or absent fields remain unknown. Distinct backpacks with the same displayed title cannot be reliably distinguished: the latest observation replaces that title's prior observation. Collector observations reset when the game restarts; saved files can remain readable until their 24-hour expiry. No guarantee of complete item upgrades is made.

Collector 0.2.0 also captures current owner inventory slots 0–40 and selected typed numeric facts from supported manually opened progression menus. It extracts stored-weapon name/stars from tool lore without exporting the lore itself. It adds hashed item instance IDs, selected scroll/gem/attribute/fuel fields and explicit partial coverage. See [factual data coverage](FACTUAL-DATA-COVERAGE.md) for source limits and unsupported fields.

## Rules assessment

Reviewed 11 October 2026 against [Hypixel Allowed Modifications](https://support.hypixel.net/hc/en-us/articles/6472550754962-Hypixel-Allowed-Modifications), [SkyBlock Rules](https://support.hypixel.net/hc/en-us/articles/4508088842898-Hypixel-SkyBlock-Rules) and the [API Policy](https://developer.hypixel.net/policies/) updated 30 September 2026. The design is restricted to recording the owner's already visible status and menu data, consistent with the purposes of permitted status displays and gameplay recorders. This is our rules assessment, not an official ruling. Hypixel says all modifications are used at the player's own risk and offers no guarantees; modifications outside its permitted categories are disallowed by default.

The collector has no automation, combat assistance, hidden-player information, de-anonymization, server-protocol changes or API requests. The optional bridge uses the operator's verified SSH connection to their own server; it does not access Minecraft networking or expose a public upload endpoint. Authenticated Hypixel requests remain in the registered MCP service with caching and rate limits. Public distribution does not share the maintainer's machine, key or tunnel. API application approval would not itself approve the mod.

## Startup correction

Collector 0.1.0 fails during Fabric startup because its mixin package includes the ordinary client entrypoint. Replace it with 0.1.1, which keeps accessors in a dedicated `dev.blockcontext.client.mixin` package. Keep only one collector JAR in `mods`. Backend 0.9.1 accepts 0.1.0, 0.1.1, 0.2.0, 0.2.1 0.2.2 and 0.3.1 snapshots.

## Build and install

Collector 0.3.7 targets Minecraft **26.2**, Java **25**, Fabric Loader **0.19.5** and Fabric API **0.161.0+26.2**. Other game versions are not validated. The MCP server remains usable with standard stdio and Streamable HTTP clients on their supported systems.

With an existing Prism Launcher instance containing those dependencies:

```sh
python client-mod/build.py --prism-root /absolute/path/to/PrismLauncher --instance your-instance-name
```

The build compiles against your local Minecraft libraries, runs the parser checks, and packages only BlockContext classes/resources and the MIT license. It does not redistribute Minecraft, Fabric or other mods. Put `client-mod/build/BlockContext-Collector-0.3.1-mc26.2.jar` in that instance's `minecraft/mods` directory, then restart Minecraft. Removing this JAR and restarting disables collection. Existing mods are unaffected.

Join SkyBlock and open the menus manually. Leave each open for at least two seconds. The local output is `minecraft/blockcontext/latest.json`; on POSIX systems the directory is mode 700 and file 600. On Windows, restrict its folder/file ACLs to your account. Only an unambiguous SkyBlock profile on a `hypixel.net` server is collected. Do not send the file to a public chat or issue.

## MCP integration

For an MCP instance on the same machine, set the absolute `BLOCKCONTEXT_CLIENT_SNAPSHOT_PATH` in its private environment file and restart that instance. `skyblock_client_context` reads the latest client observations without calling the Hypixel API. Standard overview and relevant section responses also include a separate `clientObservations` object. Client data never replaces an API bank/equipment value or changes its coverage.

Each observation has its own timestamp, source, age and coverage. Beyond five minutes it is marked stale; beyond 24 hours it is omitted. A recent export does not refresh an older menu observation. Timers preserve what the game displayed; the server does not decrement them or invent an expiry because effects can pause. No inference of inactive buffs or empty storage is made from missing data.

For your own remote server, the optional native Rust bridge validates and projects the file before sending it over SSH with strict host-key validation. It uses the explicitly configured destination and identity, and ignores unrelated system/user SSH proxy configuration. Use a real hostname or IP address rather than an alias that requires SSH configuration. Its private mode-600 configuration contains `source`, `ownerUuid`, `sshHost` and `sshIdentity`. Keep it outside the repository. The bridge rejects other players, oversized files, unsafe permissions, malformed data and unsupported fields. It runs independently of the mod and sends only changed latest snapshots. A local stdio setup needs no bridge.

The supplied import helper is an example for a Linux server with a `blockcontext` service account. Install a root-owned shell wrapper that runs `/opt/blockcontext/bin/blockcontext import-snapshot` without forwarding arguments, configure `/etc/blockcontext/client-owner` privately, and expose the fixed command `/usr/local/bin/blockcontext-import-client-snapshot` only to your trusted SSH operator. The destination is `/var/lib/blockcontext/client/latest.json`; it contains one latest snapshot, with no backups or observation history. Do not expose the importer to the Internet. Adapt paths and accounts deliberately for your own server.

## Verification

`cargo test --locked --all-targets` verifies MCP bounds, privacy, profile binding, freshness, bridge projection and transport behavior. The Java build runs offline menu-parser checks and rejects entrypoints or ordinary classes inside reserved mixin packages. `python client-mod/startup-check.py --prism-root /absolute/path/to/PrismLauncher --instance your-instance-name` also checks the built JAR with the actual Fabric transformer and Minecraft classes in an isolated directory, without launching Minecraft or connecting to a server. These tests do not replace a real game restart and manually observed menu test. Runtime compatibility and actual menu formats must be verified on the intended instance.

UI protocol details were checked against the primary [SkyHanni](https://github.com/hannibal002/SkyHanni) and [Skyblocker](https://github.com/SkyblockerMod/Skyblocker) sources, and [Coflnet's Huntaxe menu listener](https://github.com/Coflnet/SkyPlayerState/blob/3628ae74d99e5a857b74ecbe642b5158a495cf8d/Services/Listener/WeaponInHuntaxeListener.cs). Their implementation code is not included in this mod.

Collector 0.2.1 recognizes the observed `Prime Huntaxe - Nex Titanum` menu title as well as the legacy `Huntaxe` title. It reads only the weapon in slot 22 with a recognized weapon rarity/type footer. Placeholder slots and similarly named trade menus remain excluded. The weapon is observed passively; the collector never clicks “take” or any other control.

Collector 0.2.2 recognises all five Huntaxe titles listed by [Hypixel’s official item resource](https://api.hypixel.net/v2/resources/skyblock/items), checked 11 October 2026: Worn Huntaxe - Genesis, Sharpened Huntaxe - Dominus, Reinforced Huntaxe - Cursus, Savage Huntaxe - Praedator and Prime Huntaxe - Nex Titanum. Legacy `Huntaxe` titles remain supported. Slot and footer checks are shared across tiers; unrecognised future titles remain unsupported until verified. The observation belongs to the active profile, not an inferred separate weapon per Huntaxe.

## Live collector reads (0.3.1)

`skyblock_live_context` reads only currently observed owner data. It has no identity lookup, API fallback or gameplay request. The default maximum observation/export age is ten seconds (configurable 2–300). Poll no faster than once per second. Collector exports every second; the supplied private SSH bridge checks every two seconds using a private reusable SSH control connection. Export time does not refresh old observations: current keys must also appear in `observedNow`. Closed menus are excluded from live responses, while `skyblock_client_context` can return dated prior observations.

Current inventory—including a carried Huntaxe—needs no menu opening. Safely projected structured item attributes, primary displayed item stats, selected slot, the owner's Minecraft health/XP/effects, and displayed sidebar/action-bar/tab facts are captured passively. Minecraft health/XP/effects are labelled separately from SkyBlock stats and custom potion status. Effective stats remain conditional on the observed setup/context; no combat, movement or item-use input occurs. A Huntaxe summary and full absorbed-item record are different evidence. The summary does not recreate missing enchantments; full details require the server to send the stored item.

Supported progression menus additionally preserve safe item attributes and numeric fields. The latest 64 observed numeric/identifier/boolean changes are retained per profile during the current collector run; the next exported snapshot carries this bounded change window. These are changes in visibility, never proof of sales, ownership loss, purchases or completed dailies. No separate historical database is created. A restart resets this window. Private raw identifiers, arbitrary chat, arbitrary lore, credentials, other-player records and server addresses are excluded.

The export is bounded to 1 MiB and 96 observation keys per profile. `evictedObservations`, per-item attribute truncation/omissions, freshness and pagination declare incomplete coverage. No passive client can supply profile state the server has not transmitted. Menus may still be needed for bank accounts, remote storage and other server-unsent facts; unsupported/changed menu formats fail closed.

## Commission investigation build (0.3.2)

This collector build adds `commissionMenuEvidence` from a manually opened `Commissions` menu. It retains bounded slot/name records, explicit claimable/completed/daily markers and numeric evidence with line positions and broad reward/progress categories. Arbitrary lore and chat text are excluded. Unclassified numbers have no verified counter meaning. The record declares `claimsVerified: false` and `completeSystem: false`; neither a completed task nor a reward preview proves a successful claim. These records do not populate a daily claim count or reset time.

Backend candidate `0.9.2-dev.0` accepts this evidence through the local reader and private SSH projection. It is an investigation build, pending a real menu/claim comparison. Existing commission bonus-presence observations keep their separate timestamps and uncertainty. No new message capture, server request, gameplay input or API polling is introduced.

## Observed commission tracking (0.3.3)

Live testing distinguished completion from claiming: milestone progress advances before the claim. The tracker never treats that counter or HotM XP gains as claim acknowledgements. It samples only the owner's already opened Commissions menu each client tick; exports remain once per second. Stable transitions from completed/claimable to a fresh task establish observed claim transitions. Partial updates, invalid reward formats, profile changes, menu/disconnect gaps and long sampling gaps cannot establish a claim. Counts cover observations in the collector run, not every claim today, and reset when Minecraft restarts.

Two or more validated commission reward previews without a Daily bonus establish zero remaining at that observation. Empty menus, placeholders or incomplete rewards do not. A bonus preview establishes availability but leaves its positive remaining count unknown. A later return of the bonus after validated exhaustion records a replenishment interval; it does not establish an exact reset time or a full starting count. No fixed midnight, timezone or four-charge inference is applied.

These are game-derived observations, with no personal goals/preferences, raw lore, player chat or gameplay inputs. Saved menu data remains dated, and a closed menu does not establish current bonus state. `usedCharges` stays unknown because the observed transition count may omit earlier or unseen claims. The local and remote readers validate the new scalar evidence independently of the API.

## Commission display compatibility (0.3.4)

The live reward tooltip uses list prefixes (`- +410 HOTM Exp`, `- 500 Mithril Powder`) and a decorated progress bar followed by a percentage. Collector 0.3.4 accepts these anchored formats as well as the earlier undecorated forms. Reward amounts must be positive and bounded; progress must be between 0 and 100. Duplicate rewards/progress, unexpected text and missing evidence remain unvalidated. The claim tracker only accepts a zero from the parsed progress field, never an unrelated percentage. No raw tooltip text is exported.

## Persistent commission ledger (0.3.5)

Observed claim totals, bonus-claim totals, last count observations and bonus-replenishment windows are checkpointed per Minecraft UUID and profile in `blockcontext/commission-ledger.bin`. This is a bounded latest checkpoint (eight profiles, checksum, private atomic writes), not a chat/event archive. Restart recovery preserves totals but discards pending UI candidates and current positive-count coverage. Corrupt, future or insecure checkpoints are preserved and reported unavailable rather than overwritten. Ordinary observations expire after 24 hours; the latest dated partial ledger can be read for up to 365 days. Minecraft keeps its bounded checkpoint across longer absences.

An explicit displayed remaining counter can anchor an exact count while subsequent bonus claims are continuously observed. Restart, disconnect, profile/menu changes, sampling gaps, inconsistent previews or a reference-day change invalidate that current positive count. Validated current reward exhaustion still establishes zero. A reward marked Daily alone never establishes four remaining. Unseen claims remain unknown.

The reference schedule is fixed 05:00 UTC, based on NEU's published implementation, with a historical official four-bonus rule. It is identified as a community reset reference, not a server-confirmed schedule or proof of four current bonuses. `commissionTracking` is pollable tracker state and always has `currentStateVerified: false`; its live metadata may separately verify that the tracker is connected. Reference-day counters are partial observations classified against that model. Bonus replenishment retains an observation window and whether it contains the expected boundary; it does not supply an exact server timestamp.

See [deferred checks](DEFERRED-CHECKS.md) to attach a due commission check to the next explicit profile snapshot request without creating a background poll.

### Short reward refreshes (0.3.6)

A real claim exposed a brief partially updated reward preview while the Commissions screen stayed open. The collector now keeps its preceding stable claimable observation for at most two seconds while that same owner menu's data is incomplete. Incomplete previews still produce no claim or exhaustion result; a later stable validated fresh task must corroborate the transition. Longer loading gaps, closed menus, disconnects, profile changes and restarts discard the anchor. Any incomplete update invalidates current positive-count coverage. No click listener, input automation or message capture is added.

Run the bridge with `blockcontext bridge --config /absolute/private/path/client-bridge.json`. The private JSON configuration contains `source`, `ownerUuid`, `sshHost` and `sshIdentity`; require Unix mode 0600. It uploads changed latest snapshots, never polls Hypixel, and never controls the game.

The native bridge accepts an optional absolute `logPath` in its private JSON configuration. Use a separate directory and `usage.jsonl` basename, then run the report with `--component bridge`. It records only performance/resource measurements and fixed upload outcome counters. It sends no metrics to the server. See [LOGGING.md](LOGGING.md) and [the optimisation runbook](OPTIMISATION.md).


## Experimentation Table usage (0.3.7 / backend 0.11.4)

Read `skyblock_profile_section` with `section: "dailies"` and inspect the `experimentation` activity. Its `apiUsage.games` separates Superpairs (`pairings`), Chronomatron (`simon`) and Ultrasequencer (`numbers`), with per-tier reported claims/attempts and a subtotal of reported claim counters. Missing tiers stay absent, and the subtotal does not certify all uses or today's total. Raw claimed flags, last-attempt values, charge timestamps and paid-reset counters are reported without assuming daily or timer semantics.

The passive `experimentTable` observation reads only the manually opened main `Experimentation Table`/`Experiments` menu, never the puzzle grids. It recognises `Charges: 0/3`, `Next charge in: 23h` and the `Renew Experiments` tooltip's `You've renewed 0/3 charges today!`. Current available charges, displayed capacity, paid renewals used and remaining, and recharge/paid-reset durations stay separately sourced. Remaining renewals are the explicit displayed limit minus the displayed used count. A cooldown marker is not treated as a completed experiment. A rounded `23h` display remains the duration shown, not an exact UTC reset or a live countdown. Unsupported or conflicting lines remain unknown.

`experimentTracking` exposes persistent observed net charge decreases/increases. It is a partial accounting checkpoint, not a count of completed/claimed Superpairs or all experiments today. Replenishment can mask spending. Baselines are invalidated on restart, disconnect/profile change and long sampling gaps; the counters already observed survive. Closing and returning to the table during continuous play can establish a net change, but never an unseen event history. The current menu is required to establish fresh available charges; a fresh tracker heartbeat does not refresh an older menu value.

Checkpoints are stored privately per owner UUID/profile in `blockcontext/experiment-ledger.bin`, using a checksum, bounded eight-profile storage and atomic account-only writes. No pending baseline survives restart. Corrupt, future or insecure files are preserved and persistence is marked unavailable. There is no background API polling, reward/chat capture, puzzle solver or gameplay action. Saved ordinary observations last up to 24 hours; the dated partial ledger is available for up to 365 days. Use `skyblock_live_context` with `section: "dailies"` for client-only polls; it makes zero API requests. `usesToday`, `usedCharges`, `completedToday` and an exact reset remain unknown unless a future verified source supplies them.
