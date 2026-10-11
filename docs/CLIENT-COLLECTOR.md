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

Collector 0.3.1 targets Minecraft **26.2**, Java **25**, Fabric Loader **0.19.5** and Fabric API **0.161.0+26.2**. Other game versions are not validated. The MCP server remains usable with standard stdio and Streamable HTTP clients on their supported systems.

With an existing Prism Launcher instance containing those dependencies:

```sh
python client-mod/build.py --prism-root /absolute/path/to/PrismLauncher --instance your-instance-name
```

The build compiles against your local Minecraft libraries, runs the parser checks, and packages only BlockContext classes/resources and the MIT license. It does not redistribute Minecraft, Fabric or other mods. Put `client-mod/build/BlockContext-Collector-0.3.1-mc26.2.jar` in that instance's `minecraft/mods` directory, then restart Minecraft. Removing this JAR and restarting disables collection. Existing mods are unaffected.

Join SkyBlock and open the menus manually. Leave each open for at least two seconds. The local output is `minecraft/blockcontext/latest.json`; on POSIX systems the directory is mode 700 and file 600. On Windows, restrict its folder/file ACLs to your account. Only an unambiguous SkyBlock profile on a `hypixel.net` server is collected. Do not send the file to a public chat or issue.

## MCP integration

For an MCP instance on the same machine, set the absolute `BLOCKCONTEXT_CLIENT_SNAPSHOT_PATH` in its private environment file and restart that instance. `skyblock_client_context` reads the latest client observations without calling the Hypixel API. Standard overview and relevant section responses also include a separate `clientObservations` object. Client data never replaces an API bank/equipment value or changes its coverage.

Each observation has its own timestamp, source, age and coverage. Beyond five minutes it is marked stale; beyond 24 hours it is omitted. A recent export does not refresh an older menu observation. Timers preserve what the game displayed; the server does not decrement them or invent an expiry because effects can pause. No inference of inactive buffs or empty storage is made from missing data.

For your own remote server, the optional Python bridge validates and projects the file before sending it over SSH with strict host-key validation. It uses the explicitly configured destination and identity, and ignores unrelated system/user SSH proxy configuration. Use a real hostname or IP address rather than an alias that requires SSH configuration. Its private mode-600 configuration contains `source`, `ownerUuid`, `sshHost` and `sshIdentity`. Keep it outside the repository. The bridge rejects other players, oversized files, unsafe permissions, malformed data and unsupported fields. It runs independently of the mod and sends only changed latest snapshots. A local stdio setup needs no bridge.

The supplied import helper is an example for a Linux server with a `blockcontext` service account. Install the helper and `client_snapshot.py` in a root-owned directory, configure `/etc/blockcontext/client-owner` privately, and expose the fixed command `/usr/local/bin/blockcontext-import-client-snapshot` only to your trusted SSH operator. The destination is `/var/lib/blockcontext/client/latest.json`; it contains one latest snapshot, with no backups or observation history. Do not expose the importer to the Internet. Adapt paths and accounts deliberately for your own server.

## Verification

`npm run check` verifies MCP bounds, privacy, profile binding, freshness and transport behavior. `python -m unittest discover -s test -p '*_test.py'` verifies bridge projection, account isolation, rejected inputs and expiry. The Java build runs offline menu-parser checks and rejects entrypoints or ordinary classes inside reserved mixin packages. `python client-mod/startup-check.py --prism-root /absolute/path/to/PrismLauncher --instance your-instance-name` also checks the built JAR with the actual Fabric transformer and Minecraft classes in an isolated directory, without launching Minecraft or connecting to a server. These tests do not replace a real game restart and manually observed menu test. Runtime compatibility and actual menu formats must be verified on the intended instance.

UI protocol details were checked against the primary [SkyHanni](https://github.com/hannibal002/SkyHanni) and [Skyblocker](https://github.com/SkyblockerMod/Skyblocker) sources, and [Coflnet's Huntaxe menu listener](https://github.com/Coflnet/SkyPlayerState/blob/3628ae74d99e5a857b74ecbe642b5158a495cf8d/Services/Listener/WeaponInHuntaxeListener.cs). Their implementation code is not included in this mod.

Collector 0.2.1 recognizes the observed `Prime Huntaxe - Nex Titanum` menu title as well as the legacy `Huntaxe` title. It reads only the weapon in slot 22 with a recognized weapon rarity/type footer. Placeholder slots and similarly named trade menus remain excluded. The weapon is observed passively; the collector never clicks “take” or any other control.

Collector 0.2.2 recognises all five Huntaxe titles listed by [Hypixel’s official item resource](https://api.hypixel.net/v2/resources/skyblock/items), checked 11 October 2026: Worn Huntaxe - Genesis, Sharpened Huntaxe - Dominus, Reinforced Huntaxe - Cursus, Savage Huntaxe - Praedator and Prime Huntaxe - Nex Titanum. Legacy `Huntaxe` titles remain supported. Slot and footer checks are shared across tiers; unrecognised future titles remain unsupported until verified. The observation belongs to the active profile, not an inferred separate weapon per Huntaxe.

## Live collector reads (0.3.1)

`skyblock_live_context` reads only currently observed owner data. It has no identity lookup, API fallback or gameplay request. The default maximum observation/export age is ten seconds (configurable 2–300). Poll no faster than once per second. Collector exports every second; the supplied private SSH bridge checks every two seconds using a private reusable SSH control connection. Export time does not refresh old observations: current keys must also appear in `observedNow`. Closed menus are excluded from live responses, while `skyblock_client_context` can return dated prior observations.

Current inventory—including a carried Huntaxe—needs no menu opening. Safely projected structured item attributes, primary displayed item stats, selected slot, the owner's Minecraft health/XP/effects, and displayed sidebar/action-bar/tab facts are captured passively. Minecraft health/XP/effects are labelled separately from SkyBlock stats and custom potion status. Effective stats remain conditional on the observed setup/context; no combat, movement or item-use input occurs. A Huntaxe summary and full absorbed-item record are different evidence. The summary does not recreate missing enchantments; full details require the server to send the stored item.

Supported progression menus additionally preserve safe item attributes and numeric fields. The latest 64 observed numeric/identifier/boolean changes are retained per profile during the current collector run; the next exported snapshot carries this bounded change window. These are changes in visibility, never proof of sales, ownership loss, purchases or completed dailies. No separate historical database is created. A restart resets this window. Private raw identifiers, arbitrary chat, arbitrary lore, credentials, other-player records and server addresses are excluded.

The export is bounded to 1 MiB and 96 observation keys per profile. `evictedObservations`, per-item attribute truncation/omissions, freshness and pagination declare incomplete coverage. No passive client can supply profile state the server has not transmitted. Menus may still be needed for bank accounts, remote storage and other server-unsent facts; unsupported/changed menu formats fail closed.
