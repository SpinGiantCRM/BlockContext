package dev.blockcontext.client;

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
import net.minecraft.world.item.component.CustomData;
import net.minecraft.world.item.component.ItemLore;
import net.minecraft.nbt.CompoundTag;
import java.nio.file.*;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicReference;

/** Records displayed owner data. Deliberately has no HTTP, packet, chat or gameplay APIs. */
public final class BlockContextClient implements ClientModInitializer {
    private static final Gson JSON=new GsonBuilder().serializeNulls().create();
    private final Map<String,Map<String,Object>> profiles=new LinkedHashMap<>();
    private final AtomicReference<byte[]> pending=new AtomicReference<>();
    private final ScheduledExecutorService writer=Executors.newSingleThreadScheduledExecutor(r->{Thread t=new Thread(r,"BlockContext snapshot writer");t.setDaemon(true);return t;});
    private Path output;
    private int ticks;
    private long lastWrite;
    private String lastProfile;
    private boolean wasActive;
    @Override public void onInitializeClient() {
        output=FabricLoader.getInstance().getGameDir().resolve("blockcontext/latest.json");
        writer.scheduleWithFixedDelay(this::writePending,0,1,TimeUnit.SECONDS);
        ClientTickEvents.END_CLIENT_TICK.register(this::tick);
        ClientLifecycleEvents.CLIENT_STOPPING.register(client->{snapshot(false);writePending();writer.shutdown();});
        System.out.println("[BlockContext] Passive local collector loaded; no game actions or external networking.");
    }
    private void tick(Minecraft client) {
        if(++ticks%20!=0)return;
        try { collect(client); } catch(Exception ignored) { /* Fail closed; never dump UI data or credentials. */ }
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
        Map<String,Object> state=profiles.computeIfAbsent(key,k->{Map<String,Object> p=new LinkedHashMap<>();p.put("uuid",uuid);p.put("playerName",name);p.put("profileName",profile);p.put("observations",new LinkedHashMap<String,Object>());return p;});
        while(profiles.size()>8)profiles.remove(profiles.keySet().iterator().next());
        @SuppressWarnings("unchecked") Map<String,Object> observations=(Map<String,Object>)state.get("observations");
        long now=System.currentTimeMillis();
        for(var e:MenuParser.tabBuffs(lines).entrySet())observe(observations,e.getKey(),e.getValue(),"tab_display",now);
        if(client.gui.screen() instanceof AbstractContainerScreen<?> screen&&screen.getMenu() instanceof ChestMenu menu) {
            String title=MenuParser.clean(screen.getTitle().getString()),kind=MenuParser.kind(title);
            if(kind!=null) {
                List<MenuParser.ItemView> items=new ArrayList<>();int count=Math.min(54,menu.getRowCount()*9);
                for(int slot=0;slot<count;slot++)items.add(item(menu.getSlot(slot).getItem(),slot));
                Map<String,Object> value=MenuParser.parse(title,items);
                if(kind.equals("buffs"))for(var e:value.entrySet()) { @SuppressWarnings("unchecked") Map<String,Object> v=(Map<String,Object>)e.getValue();observe(observations,e.getKey(),v,"opened_menu",now); }
                else if(kind.equals("bank"))for(var e:value.entrySet())observe(observations,"bank:"+e.getKey(),Map.of(e.getKey(),e.getValue()),"opened_menu",now);
                else if(!value.isEmpty())observe(observations,kind.equals("storage")?"storage:"+title:kind,value,"opened_menu",now);
            }
        }
        // Never retain an unbounded menu history. Each key holds only its latest observation.
        while(observations.size()>24)observations.remove(observations.keySet().iterator().next());
        lastProfile=key;wasActive=true;
        if(now-lastWrite>=5000){state.put("lastSeenAt",now);snapshot(true);lastWrite=now;}
    }
    private void observe(Map<String,Object> observations,String key,Map<String,Object> value,String from,long now) {
        observations.put(key,Map.of("observedAt",now,"readFrom",from,"values",value));
    }
    private MenuParser.ItemView item(ItemStack stack,int slot) {
        CustomData custom=stack.get(DataComponents.CUSTOM_DATA);CompoundTag data=custom==null?new CompoundTag():custom.copyTag();
        CompoundTag extra=data.getCompound("ExtraAttributes").orElse(data);String id=extra.getString("id").orElse(null);
        if(id!=null&&!id.matches("[A-Z0-9_:.-]{1,96}"))id=null;
        Map<String,Object> out=new LinkedHashMap<>();out.put("slot",slot);out.put("id",id);out.put("count",stack.getCount());
        String modifier=extra.getString("modifier").orElse(null);if(modifier!=null&&modifier.matches("[a-z0-9_]{1,64}"))out.put("reforge",modifier);
        for(String key:List.of("hot_potato_count","rarity_upgrades","upgrade_level","dungeon_item_level","art_of_war_count","farming_for_dummies_count","wood_singularity_count","tuned_transmission"))extra.getInt(key).filter(n->n>=0&&n<=100).ifPresent(n->out.put(key,n));
        Map<String,Integer> enchants=new LinkedHashMap<>();var enchant=extra.getCompoundOrEmpty("enchantments");
        for(var e:enchant.entrySet())if(enchants.size()<128&&e.getKey().matches("[a-z0-9_]{1,64}"))enchant.getInt(e.getKey()).filter(n->n>0&&n<=100).ifPresent(n->enchants.put(e.getKey(),n));out.put("enchantments",enchants);
        List<String> lore=new ArrayList<>();ItemLore itemLore=stack.get(DataComponents.LORE);if(itemLore!=null)for(var line:itemLore.lines()){if(lore.size()>=64)break;String s=line.getString();if(s.length()<=256)lore.add(s);}
        return new MenuParser.ItemView(slot,id,stack.getHoverName().getString(),lore,out);
    }
    private void inactive(){if(wasActive){wasActive=false;snapshot(false);}lastProfile=null;}
    private void snapshot(boolean active) {
        if(profiles.isEmpty())return;
        Map<String,Object> root=new LinkedHashMap<>();root.put("schemaVersion",1);root.put("collectorVersion","0.1.0");root.put("exportedAt",System.currentTimeMillis());root.put("activeProfile",active?lastProfile:null);root.put("profiles",new ArrayList<>(profiles.values()));
        byte[] bytes=JSON.toJson(root).getBytes(java.nio.charset.StandardCharsets.UTF_8);if(bytes.length<=512*1024)pending.set(bytes);
    }
    private synchronized void writePending() {
        byte[] bytes=pending.getAndSet(null);if(bytes==null)return;
        Path temp=null;
        try {
            Files.createDirectories(output.getParent());permissions(output.getParent(),"rwx------");
            if(Files.isSymbolicLink(output.getParent())||Files.isSymbolicLink(output))return;
            boolean posix=Files.getFileStore(output.getParent()).supportsFileAttributeView("posix");
            temp=posix?Files.createTempFile(output.getParent(),".snapshot-",".tmp",PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rw-------"))):Files.createTempFile(output.getParent(),".snapshot-",".tmp");
            Files.write(temp,bytes,StandardOpenOption.TRUNCATE_EXISTING);
            Files.move(temp,output,StandardCopyOption.REPLACE_EXISTING,StandardCopyOption.ATOMIC_MOVE);
        }catch(Exception ignored){/* Owner data never enters a log on I/O failure. */}finally{if(temp!=null)try{Files.deleteIfExists(temp);}catch(Exception ignored){}}
    }
    private static void permissions(Path p,String mode) throws java.io.IOException {if(Files.getFileStore(p).supportsFileAttributeView("posix"))Files.setPosixFilePermissions(p,PosixFilePermissions.fromString(mode));}
}
