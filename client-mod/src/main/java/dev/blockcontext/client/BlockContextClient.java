package dev.blockcontext.client;

import dev.blockcontext.client.mixin.TabOverlayAccess;
import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.inventory.ChestMenu;
import net.minecraft.world.scores.DisplaySlot;
import net.minecraft.world.item.ItemStack;
import java.security.MessageDigest;
import net.minecraft.world.item.component.CustomData;
import net.minecraft.world.item.component.ItemLore;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.Tag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NumericTag;
import net.minecraft.world.scores.PlayerTeam;
import net.fabricmc.fabric.api.client.message.v1.ClientReceiveMessageEvents;
import java.nio.file.*;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicReference;

/** Records displayed owner data. Deliberately has no HTTP, packet, chat or gameplay APIs. */
public final class BlockContextClient implements ClientModInitializer {
    private static final Gson JSON=new GsonBuilder().serializeNulls().create();
    private final Map<String,Map<String,Object>> profiles=new LinkedHashMap<>();
    private final Map<String,CommissionTracker> commissionTrackers=new LinkedHashMap<>();
    private final AtomicReference<byte[]> pending=new AtomicReference<>();
    private final AtomicReference<byte[]> pendingLedger=new AtomicReference<>();
    private Path ledgerFile;
    private Path experimentLedgerFile;
    private final Map<String,ExperimentTracker> experimentTrackers=new LinkedHashMap<>();
    private final AtomicReference<byte[]> pendingExperimentLedger=new AtomicReference<>();
    private volatile boolean experimentPersistenceAvailable=true;
    private volatile boolean persistenceAvailable=true;
    private final ScheduledExecutorService writer=Executors.newSingleThreadScheduledExecutor(r->{Thread t=new Thread(r,"BlockContext snapshot writer");t.setDaemon(true);return t;});
    private Path output;
    private int ticks;
    private long lastWrite;
    private String lastProfile;
    private List<Map<String,Object>> hudFacts=List.of();
    private long hudObservedAt;
    private String hudProfile;
    private final Set<String> observedNow=new LinkedHashSet<>();
    private boolean wasActive;
    private int evictedObservations;
    private List<Map<String,Object>> activeChanges=new ArrayList<>();
    @Override public void onInitializeClient() {
        output=FabricLoader.getInstance().getGameDir().resolve("blockcontext/latest.json");
        ledgerFile=output.resolveSibling("commission-ledger.bin");
        experimentLedgerFile=output.resolveSibling("experiment-ledger.bin");
        try{experimentTrackers.putAll(ExperimentLedgerFile.read(experimentLedgerFile,System.currentTimeMillis()));}catch(Exception ignored){experimentPersistenceAvailable=false;System.out.println("[BlockContext] Experiment checkpoint unavailable; existing file preserved.");}
        try{commissionTrackers.putAll(CommissionLedgerFile.read(ledgerFile,System.currentTimeMillis()));}catch(Exception ignored){persistenceAvailable=false;System.out.println("[BlockContext] Commission checkpoint unavailable; existing file preserved.");}
        writer.scheduleWithFixedDelay(this::writePending,0,1,TimeUnit.SECONDS);
        ClientReceiveMessageEvents.GAME.register((message,overlay)->{
            if(overlay&&wasActive&&lastProfile!=null){hudFacts=MenuParser.hudFacts(message.getString());hudObservedAt=System.currentTimeMillis();hudProfile=lastProfile;}
        });
        ClientTickEvents.END_CLIENT_TICK.register(this::tick);
        ClientLifecycleEvents.CLIENT_STOPPING.register(client->{snapshot(false);writePending();writer.shutdown();});
        System.out.println("[BlockContext] Passive local collector loaded; no game actions or external networking.");
    }
    private void tick(Minecraft client) {
        try {
            if(++ticks%20==0)collect(client);
            collectCommissionTick(client);
        } catch(Exception ignored) { /* Fail closed; never dump UI data or credentials. */ }
    }
    private void collectCommissionTick(Minecraft client) {
        if(!wasActive||lastProfile==null||client.player==null||client.level==null||client.getConnection()==null||client.getCurrentServer()==null
            ||!client.getCurrentServer().ip.toLowerCase(Locale.ROOT).matches("(?:[a-z0-9-]+\\.)*hypixel\\.net(?::\\d{1,5})?")
            ||!(client.gui.screen() instanceof AbstractContainerScreen<?> screen)||!(screen.getMenu() instanceof ChestMenu menu)||!MenuParser.clean(screen.getTitle().getString()).equals("Commissions")){
            commissionTrackers.values().forEach(CommissionTracker::gap);return;
        }
        var objective=client.level.getScoreboard().getDisplayObjective(DisplaySlot.SIDEBAR);
        if(objective==null||!MenuParser.clean(objective.getDisplayName().getString()).startsWith("SKYBLOCK")){commissionTrackers.values().forEach(CommissionTracker::gap);return;}
        List<String> lines=new ArrayList<>();
        for(var info:client.getConnection().getOnlinePlayers()){var name=info.getTabListDisplayName();if(name!=null&&lines.size()<512)lines.addAll(Arrays.asList(name.getString().split("\\n",64)));}
        TabOverlayAccess tab=(TabOverlayAccess)client.gui.hud.getTabList();
        if(tab.blockcontext$header()!=null)lines.addAll(Arrays.asList(tab.blockcontext$header().getString().split("\\n",128)));
        if(tab.blockcontext$footer()!=null)lines.addAll(Arrays.asList(tab.blockcontext$footer().getString().split("\\n",128)));
        String profile=MenuParser.profile(lines);
        String key=client.getGameProfile().id().toString().replace("-","")+":"+(profile==null?"":profile.toLowerCase(Locale.ROOT));
        if(profile==null||!key.equals(lastProfile)){commissionTrackers.values().forEach(CommissionTracker::gap);return;}
        var state=profiles.get(key);if(state==null)return;
        @SuppressWarnings("unchecked") var observations=(Map<String,Object>)state.get("observations");
        List<MenuParser.ItemView> items=new ArrayList<>();int count=Math.min(54,menu.getRowCount()*9);
        for(int slot=0;slot<count;slot++){
            var stack=menu.getSlot(slot).getItem();String name=MenuParser.clean(stack.getHoverName().getString());
            if(!(slot>=9&&slot<18)&&!name.equals("Commission Milestones"))continue;
            List<String> lore=new ArrayList<>();var received=stack.get(DataComponents.LORE);
            if(received!=null)for(var line:received.lines()){if(lore.size()>=64)break;String text=line.getString();if(text.length()<=256)lore.add(text);}
            items.add(new MenuParser.ItemView(slot,null,name,lore,Map.of()));
        }
        long now=System.currentTimeMillis();var evidence=MenuParser.commissionEvidence(items);
        var tracker=commissionTrackers.computeIfAbsent(key,k->new CommissionTracker());var values=new LinkedHashMap<>(MenuParser.commissionMenu(items));values.putAll(tracker.sample(evidence,now,values));
        if(!values.isEmpty())observe(observations,"commissions",values,"opened_menu",now);
        observe(observations,"commissionMenuEvidence",evidence,"opened_menu",now);
    }
    private void collect(Minecraft client) {
        var server=client.getCurrentServer();var connection=client.getConnection();
        if(server==null||client.player==null||client.level==null||connection==null||!server.ip.toLowerCase(Locale.ROOT).matches("(?:[a-z0-9-]+\\.)*hypixel\\.net(?::\\d{1,5})?")) { inactive();return; }
        var objective=client.level.getScoreboard().getDisplayObjective(DisplaySlot.SIDEBAR);
        if(objective==null||!MenuParser.clean(objective.getDisplayName().getString()).startsWith("SKYBLOCK")){inactive();return;}
        List<String> lines=new ArrayList<>();
        for(var info:connection.getOnlinePlayers()) {var name=info.getTabListDisplayName();if(name!=null&&lines.size()<512)lines.addAll(Arrays.asList(name.getString().split("\\n",64)));}
        TabOverlayAccess tab=(TabOverlayAccess)client.gui.hud.getTabList();
        if(tab.blockcontext$header()!=null)lines.addAll(Arrays.asList(tab.blockcontext$header().getString().split("\\n",128)));
        if(tab.blockcontext$footer()!=null)lines.addAll(Arrays.asList(tab.blockcontext$footer().getString().split("\\n",128)));
        String profile=MenuParser.profile(lines);if(profile==null){inactive();return;}
        String uuid=client.getGameProfile().id().toString().replace("-","");String name=client.getGameProfile().name();
        if(!name.matches("[A-Za-z0-9_]{1,16}")) {inactive();return;}
        String key=uuid+":"+profile.toLowerCase(Locale.ROOT);
        if(lastProfile!=null&&!key.equals(lastProfile)){commissionTrackers.values().forEach(CommissionTracker::gap);experimentTrackers.values().forEach(ExperimentTracker::gap);}
        Map<String,Object> state=profiles.computeIfAbsent(key,k->{Map<String,Object> p=new LinkedHashMap<>();p.put("uuid",uuid);p.put("playerName",name);p.put("profileName",profile);p.put("observations",new LinkedHashMap<String,Object>());return p;});
        while(profiles.size()>8)profiles.remove(profiles.keySet().iterator().next());
        @SuppressWarnings("unchecked") Map<String,Object> observations=(Map<String,Object>)state.get("observations");
        @SuppressWarnings("unchecked") var changes=(List<Map<String,Object>>)state.computeIfAbsent("changes",k->new ArrayList<Map<String,Object>>());activeChanges=changes;
        long now=System.currentTimeMillis();observedNow.clear();
        List<String> sidebar=new ArrayList<>();var scoreboard=client.level.getScoreboard();
        for(var score:scoreboard.listPlayerScores(objective))if(!score.isHidden()&&sidebar.size()<15){sidebar.add(PlayerTeam.formatNameForTeam(scoreboard.getPlayersTeam(score.owner()),score.ownerName()).getString());}
        var sidebarFacts=MenuParser.numericFacts(sidebar);
        if(!sidebarFacts.isEmpty())observe(observations,"sidebar",Map.of("facts",List.of(Map.of("slot",0,"displayName","SkyBlock sidebar","fields",sidebarFacts)),"observedSlots",0,"completeSystem",false),"sidebar_display",now);
        if(key.equals(hudProfile)&&now-hudObservedAt<=5000&&!hudFacts.isEmpty()) {
            observe(observations,"actionBar",Map.of("facts",List.of(Map.of("slot",0,"displayName","SkyBlock HUD","fields",hudFacts)),"observedSlots",0,"completeSystem",false),"action_bar",hudObservedAt);
        }
        Map<String,Object> stateValues=new LinkedHashMap<>();stateValues.put("selectedSlot",client.player.getInventory().getSelectedSlot());stateValues.put("minecraftHealth",client.player.getHealth());stateValues.put("minecraftMaxHealth",client.player.getMaxHealth());stateValues.put("minecraftExperienceLevel",client.player.experienceLevel);stateValues.put("minecraftExperienceProgress",client.player.experienceProgress);
        List<Map<String,Object>> effects=new ArrayList<>();for(var effect:client.player.getActiveEffects())if(effects.size()<64){var id=effect.getEffect().unwrapKey();if(id.isPresent())effects.add(Map.of("id",id.get().identifier().toString(),"amplifier",effect.getAmplifier(),"remainingTicksAtObservation",effect.getDuration(),"infinite",effect.isInfiniteDuration(),"ambient",effect.isAmbient(),"visible",effect.isVisible()));}stateValues.put("minecraftEffects",effects);
        stateValues.put("dungeonHudObserved",sidebar.stream().map(MenuParser::clean).anyMatch(l->l.matches("(?:The Catacombs|Catacombs|Dungeon).*")));
        observe(observations,"liveState",stateValues,"player_state",now);
        for(var e:MenuParser.tabBuffs(lines).entrySet())observe(observations,e.getKey(),e.getValue(),"tab_display",now);
        var commissionCounters=MenuParser.commissionCounters(lines);
        if(!commissionCounters.isEmpty())observe(observations,"commissions",commissionCounters,"tab_display",now);
        List<Map<String,Object>> ownItems=new ArrayList<>();
        for(int slot=0;slot<41;slot++){var view=item(client.player.getInventory().getItem(slot),slot);if(view.id()!=null)ownItems.add(view.item());}
        observe(observations,"inventory",Map.of("items",ownItems,"observedSlots",41,"completeStorage",false),"player_inventory",now);
        var tabFacts=MenuParser.numericFacts(lines);
        if(!tabFacts.isEmpty())observe(observations,"tabStats",Map.of("facts",List.of(Map.of("slot",0,"displayName","SkyBlock tab","fields",tabFacts)),"observedSlots",0,"completeSystem",false),"tab_display",now);
        if(client.gui.screen() instanceof AbstractContainerScreen<?> screen&&screen.getMenu() instanceof ChestMenu menu) {
            String title=MenuParser.clean(screen.getTitle().getString()),kind=MenuParser.kind(title);
            if(kind!=null) {
                List<MenuParser.ItemView> items=new ArrayList<>();int count=Math.min(54,menu.getRowCount()*9);
                for(int slot=0;slot<count;slot++)items.add(item(menu.getSlot(slot).getItem(),slot));
                if(title.equals("Commissions")){
                    var evidence=MenuParser.commissionEvidence(items);var tracker=commissionTrackers.computeIfAbsent(key,k->new CommissionTracker());
                    var commissions=new LinkedHashMap<>(MenuParser.commissionMenu(items));commissions.putAll(tracker.sample(evidence,now,commissions));
                    if(!commissions.isEmpty())observe(observations,"commissions",commissions,"opened_menu",now);
                    observe(observations,"commissionMenuEvidence",evidence,"opened_menu",now);
                }
                Map<String,Object> value=MenuParser.parse(title,items);
                if(kind.equals("experimentTable")&&!value.isEmpty())experimentTrackers.computeIfAbsent(key,k->new ExperimentTracker()).sample(value,now);
                if(kind.equals("buffs"))for(var e:value.entrySet()) { @SuppressWarnings("unchecked") Map<String,Object> v=(Map<String,Object>)e.getValue();observe(observations,e.getKey(),v,"opened_menu",now); }
                else if(kind.equals("bank"))for(var e:value.entrySet())observe(observations,"bank:"+e.getKey(),Map.of(e.getKey(),e.getValue()),"opened_menu",now);
                else if(!value.isEmpty())observe(observations,kind.equals("storage")?"storage:"+title:kind,value,"opened_menu",now);
            }
        }
        if(!observedNow.contains("commissionMenuEvidence"))commissionTrackers.values().forEach(CommissionTracker::gap);
        while(commissionTrackers.size()>8)commissionTrackers.remove(commissionTrackers.keySet().iterator().next());
        var tracking=new LinkedHashMap<>(commissionTrackers.computeIfAbsent(key,k->new CommissionTracker()).tracking(now));tracking.put("persistenceAvailable",persistenceAvailable);observe(observations,"commissionTracking",tracking,"collector_ledger",now);
        while(commissionTrackers.size()>8)commissionTrackers.remove(commissionTrackers.keySet().iterator().next());
        while(experimentTrackers.size()>8)experimentTrackers.remove(experimentTrackers.keySet().iterator().next());
        if(experimentTrackers.containsKey(key))observe(observations,"experimentTracking",experimentTrackers.get(key).tracking(experimentPersistenceAvailable),"collector_ledger",now);
        // Never retain an unbounded menu history. Each key holds only its latest observation.
        while(observations.size()>96){observations.remove(observations.keySet().iterator().next());evictedObservations++;}
        lastProfile=key;wasActive=true;
        if(now-lastWrite>=1000){state.put("lastSeenAt",now);snapshot(true);lastWrite=now;}
    }
    private void observe(Map<String,Object> observations,String key,Map<String,Object> value,String from,long now) {
        Object old=observations.get(key);
        if(old instanceof Map<?,?> record&&record.get("values") instanceof Map<?,?> values&&record.get("observedAt") instanceof Number stamp&&now-stamp.longValue()<3600000){
            @SuppressWarnings("unchecked") var before=(Map<String,Object>)values;
            activeChanges.addAll(ObservationChanges.compare(key,before,value,stamp.longValue(),now));while(activeChanges.size()>64)activeChanges.remove(0);
        }
        observedNow.add(key);observations.remove(key);observations.put(key,Map.of("observedAt",now,"readFrom",from,"values",value));
    }
    private MenuParser.ItemView item(ItemStack stack,int slot) {
        CustomData custom=stack.get(DataComponents.CUSTOM_DATA);CompoundTag data=custom==null?new CompoundTag():custom.copyTag();
        CompoundTag extra=data.getCompound("ExtraAttributes").orElse(data);String id=extra.getString("id").orElse(null);
        if(id!=null&&!id.matches("[A-Z0-9_:.-]{1,96}"))id=null;
        Map<String,Object> out=new LinkedHashMap<>();out.put("slot",slot);out.put("id",id);out.put("count",stack.getCount());
        String modifier=extra.getString("modifier").orElse(null);if(modifier!=null&&modifier.matches("[a-z0-9_]{1,64}"))out.put("reforge",modifier);
        for(String key:List.of("hot_potato_count","rarity_upgrades","upgrade_level","dungeon_item_level","art_of_war_count","art_of_peace_count","etherwarp","mana_disintegrator_count","drill_fuel","drill_fuel_capacity","fuel","pickonimbus_durability","cultivating","farmed_cultivating","compact_blocks","expertise_kills","farming_for_dummies_count","wood_singularity_count","tuned_transmission"))extra.getDouble(key).filter(n->Double.isFinite(n)&&n>=0&&n<=1e15).ifPresent(n->out.put(key,n));
        Map<String,Integer> enchants=new LinkedHashMap<>();var enchant=extra.getCompoundOrEmpty("enchantments");
        for(var e:enchant.entrySet())if(enchants.size()<128&&e.getKey().matches("[a-z0-9_]{1,64}"))enchant.getInt(e.getKey()).filter(n->n>0&&n<=100).ifPresent(n->enchants.put(e.getKey(),n));out.put("enchantments",enchants);
        for(String key:List.of("attributes","runes")) {
            var nbt=extra.getCompound(key);if(nbt.isEmpty())continue;
            Map<String,Integer> values=new LinkedHashMap<>();for(String k:nbt.get().keySet())if(values.size()<64&&k.matches("[A-Za-z0-9_:.-]{1,96}"))nbt.get().getInt(k).filter(n->n>=0&&n<=100).ifPresent(n->values.put(k,n));out.put(key,values);
        }
        if(extra.contains("ability_scroll")){List<String> scrolls=new ArrayList<>();var list=extra.getListOrEmpty("ability_scroll");for(int i=0;i<Math.min(16,list.size());i++)list.getString(i).filter(v->v.matches("[A-Z_]{1,96}")).ifPresent(scrolls::add);out.put("abilityScrolls",scrolls);}
        for(String key:List.of("drill_part_engine","drill_part_fuel_tank","drill_part_upgrade_module","skin"))extra.getString(key).filter(v->v.matches("[A-Za-z0-9_:.-]{1,96}")).ifPresent(v->out.put(key,v));
        if(extra.contains("gems")){
            Map<String,Object> gems=new LinkedHashMap<>();var nbt=extra.getCompoundOrEmpty("gems");for(String k:nbt.keySet())if(gems.size()<64&&k.matches("[A-Z0-9_]{1,64}")){
                var direct=nbt.getString(k).filter(v->v.matches("[A-Z0-9_]{1,32}"));if(direct.isPresent())gems.put(k,direct.get());
                else {Map<String,String> spec=new LinkedHashMap<>();var sub=nbt.getCompoundOrEmpty(k);for(String key:List.of("quality","type"))sub.getString(key).filter(v->v.matches("[A-Z0-9_]{1,32}")).ifPresent(v->spec.put(key,v));if(!spec.isEmpty())gems.put(k,spec);}
            }out.put("gems",gems);
        }
        @SuppressWarnings("unchecked") Map<String,Object> received=(Map<String,Object>)nbtValue(extra,0);
        if(received.get("petInfo") instanceof String petJson)try{received.put("petInfo",JSON.fromJson(petJson,Map.class));}catch(Exception ignored){received.remove("petInfo");}
        out.put("structuredAttributes",ItemFacts.project(received));
        out.put("receivedAttributeKeys",extra.keySet().stream().filter(ItemFacts::safeKey).sorted().limit(128).toList());
        String itemUuid=extra.getString("uuid").orElse(null);
        if(itemUuid!=null&&itemUuid.matches("[a-fA-F0-9-]{32,36}"))try{out.put("instanceId",HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(itemUuid.replace("-","").toLowerCase(Locale.ROOT).getBytes(java.nio.charset.StandardCharsets.UTF_8))));}catch(java.security.NoSuchAlgorithmException ignored){}
        String displayName=MenuParser.clean(stack.getHoverName().getString());if(displayName.matches("[A-Za-z0-9 '():+./%✪★-]{1,96}"))out.put("displayName",displayName);
        List<String> lore=new ArrayList<>();ItemLore itemLore=stack.get(DataComponents.LORE);if(itemLore!=null)for(var line:itemLore.lines()){if(lore.size()>=64)break;String s=line.getString();if(s.length()<=256)lore.add(s);}
        out.put("displayedStats",MenuParser.itemStats(lore));
        var stored=MenuParser.storedWeapon(lore);if(!stored.isEmpty())out.put("storedWeapon",stored);
        for(String raw:lore){String line=MenuParser.clean(raw);var match=java.util.regex.Pattern.compile("^(?:a )?(COMMON|UNCOMMON|RARE|EPIC|LEGENDARY|MYTHIC|DIVINE|SPECIAL|VERY SPECIAL) (?:DUNGEON )?([A-Z ]{1,32})(?: a)?$").matcher(line);if(match.matches()){out.put("rarity",match.group(1));out.put("itemType",match.group(2).trim());}}
        return new MenuParser.ItemView(slot,id,stack.getHoverName().getString(),lore,out);
    }
    private static Object nbtValue(Tag tag,int depth) {
        if(depth>8)return null;
        if(tag instanceof CompoundTag compound){Map<String,Object> out=new LinkedHashMap<>();for(var e:compound.entrySet())if(out.size()<128&&ItemFacts.safeKey(e.getKey()))out.put(e.getKey(),nbtValue(e.getValue(),depth+1));return out;}
        if(tag instanceof ListTag list){List<Object> out=new ArrayList<>();for(int i=0;i<Math.min(64,list.size());i++)out.add(nbtValue(list.get(i),depth+1));return out;}
        if(tag instanceof NumericTag numeric)return numeric.asNumber().orElse(null);
        return tag.asString().orElse(null);
    }
    private void inactive(){experimentTrackers.values().forEach(ExperimentTracker::gap);commissionTrackers.values().forEach(CommissionTracker::gap);if(wasActive){wasActive=false;snapshot(false);}lastProfile=null;}
    private void snapshot(boolean active) {
        if(profiles.isEmpty())return;
        if(persistenceAvailable)try{pendingLedger.set(CommissionLedgerFile.encode(commissionTrackers));}catch(Exception ignored){persistenceAvailable=false;}
        if(experimentPersistenceAvailable)try{pendingExperimentLedger.set(ExperimentLedgerFile.encode(experimentTrackers));}catch(Exception ignored){experimentPersistenceAvailable=false;}
        Map<String,Object> root=new LinkedHashMap<>();root.put("schemaVersion",1);root.put("collectorVersion","0.3.7");root.put("exportedAt",System.currentTimeMillis());root.put("activeProfile",active?lastProfile:null);root.put("profiles",new ArrayList<>(profiles.values()));root.put("observedNow",active?new ArrayList<>(observedNow):List.of());root.put("exportIntervalMs",1000);root.put("evictedObservations",evictedObservations);
        byte[] bytes=JSON.toJson(root).getBytes(java.nio.charset.StandardCharsets.UTF_8);int evicted=0;
        while(bytes.length>1024*1024&&evicted<768) {
            boolean removed=false;for(var profile:profiles.values()){
                @SuppressWarnings("unchecked") var observations=(Map<String,Object>)profile.get("observations");
                String candidate=observations.keySet().stream().filter(k->!Set.of("inventory","liveState","actionBar","sidebar","tabStats").contains(k)).findFirst().orElse(null);
                if(candidate!=null){observations.remove(candidate);evicted++;evictedObservations++;removed=true;break;}
            }if(!removed)break;root.put("evictedObservations",evictedObservations);bytes=JSON.toJson(root).getBytes(java.nio.charset.StandardCharsets.UTF_8);
        }
        if(bytes.length<=1024*1024)pending.set(bytes);
    }
    private synchronized void writePending() {
        byte[] bytes=pending.getAndSet(null);byte[] ledger=pendingLedger.getAndSet(null);byte[] experiments=pendingExperimentLedger.getAndSet(null);if(bytes==null&&ledger==null&&experiments==null)return;
        Path temp=null;
        try {
            Files.createDirectories(output.getParent());permissions(output.getParent(),"rwx------");
            if(Files.isSymbolicLink(output.getParent())||Files.isSymbolicLink(output))return;
            boolean posix=Files.getFileStore(output.getParent()).supportsFileAttributeView("posix");
            if(bytes!=null){temp=posix?Files.createTempFile(output.getParent(),".snapshot-",".tmp",PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rw-------"))):Files.createTempFile(output.getParent(),".snapshot-",".tmp");
            Files.write(temp,bytes,StandardOpenOption.TRUNCATE_EXISTING);Files.move(temp,output,StandardCopyOption.REPLACE_EXISTING,StandardCopyOption.ATOMIC_MOVE);}
            if(ledger!=null&&persistenceAvailable)try{CommissionLedgerFile.write(ledgerFile,ledger);}catch(Exception ignored){persistenceAvailable=false;}
            if(experiments!=null&&experimentPersistenceAvailable)try{CommissionLedgerFile.write(experimentLedgerFile,experiments);}catch(Exception ignored){experimentPersistenceAvailable=false;}
        }catch(Exception ignored){/* Owner data never enters a log on I/O failure. */}finally{if(temp!=null)try{Files.deleteIfExists(temp);}catch(Exception ignored){}}
    }
    private static void permissions(Path p,String mode) throws java.io.IOException {if(Files.getFileStore(p).supportsFileAttributeView("posix"))Files.setPosixFilePermissions(p,PosixFilePermissions.fromString(mode));}
}
