package dev.blockcontext.client;

import java.util.*;
import java.util.regex.*;

/** Pure projection of the owner's already displayed UI. No game actions or networking. */
public final class MenuParser {
    public record ItemView(int slot, String id, String name, List<String> lore, Map<String,Object> item) {}
    private static final Pattern DURATION = Pattern.compile("(\\d{1,7})\\s*(days?|hours?|minutes?|seconds?|[dhms])", Pattern.CASE_INSENSITIVE);
    private static final Pattern PROFILE = Pattern.compile("^Profile: ([A-Za-z][A-Za-z ]{0,31}?)(?: [^A-Za-z ].*)?$");
    // Names verified against Hypixel's public item resource. No broad title substring matching.
    private static final Set<String> HUNTAXE_TITLES = Set.of("Worn Huntaxe - Genesis", "Sharpened Huntaxe - Dominus", "Reinforced Huntaxe - Cursus", "Savage Huntaxe - Praedator", "Prime Huntaxe - Nex Titanum");
    private static final Set<String> BANK_TITLES = Set.of("Bank", "Banker", "Bank Account", "Personal Bank", "Personal Bank Account", "Co-op Bank", "Co-op Bank Account");
    private static final Set<String> GARDEN_TITLES = Set.of("Greenhouse", "Greenhouse Management", "Mutation Analysis", "Composter", "Crop Diagnostics");
    public static String clean(String value) { return value == null ? "" : value.replaceAll("§.", "").trim(); }
    public static String profile(List<String> lines) {
        Set<String> names = new HashSet<>();
        for (String line : lines) { Matcher m=PROFILE.matcher(clean(line)); if(m.matches()) names.add(m.group(1).trim()); }
        return names.size()==1 ? names.iterator().next() : null;
    }
    public static Long duration(String value) {
        value=clean(value).toLowerCase(Locale.ROOT);
        if(value.matches("\\d{1,6}:\\d{2}:\\d{2}")) { String[] p=value.split(":"); long h=Long.parseLong(p[0]),m=Long.parseLong(p[1]),s=Long.parseLong(p[2]); return m<60&&s<60?h*3600+m*60+s:null; }
        Matcher m=DURATION.matcher(value); long result=0; int end=0; Set<Character> units=new HashSet<>();
        while(m.find()) { if(!value.substring(end,m.start()).isBlank())return null; char unit=m.group(2).charAt(0); if(!units.add(unit))return null; long scale=switch(unit){case 'd'->86400;case 'h'->3600;case 'm'->60;default->1;}; result+=Long.parseLong(m.group(1))*scale; end=m.end(); }
        return end>0&&value.substring(end).isBlank()&&result<=10L*365*86400?result:null;
    }
    private static Map<String,Object> buff(boolean active, Long seconds) {
        Map<String,Object> m=new LinkedHashMap<>();m.put("active",active);m.put("remainingSecondsAtObservation",seconds);return m;
    }
    public static Map<String,Map<String,Object>> tabBuffs(List<String> lines) {
        Map<String,Map<String,Object>> out=new LinkedHashMap<>();
        for(int i=0;i<lines.size();i++) { String line=clean(lines.get(i));
            if(line.startsWith("God Potion: ")) { Long n=duration(line.substring(12));if(n!=null)out.put("godPotion",buff(n>0,n)); }
            if(line.startsWith("You have a God Potion active! ")) { Long n=duration(line.substring("You have a God Potion active! ".length()));if(n!=null)out.put("godPotion",buff(n>0,n)); }
            if(line.equals("Cookie Buff")&&i+1<lines.size()) { String next=clean(lines.get(i+1)); Long n=duration(next); if(n!=null)out.put("cookie",buff(n>0,n));else if(next.equals("Not active!")||next.equals("Not active"))out.put("cookie",buff(false,0L)); }
            if(line.startsWith("Cookie Buff: ")) { Long n=duration(line.substring(13));if(n!=null)out.put("cookie",buff(n>0,n)); }
        }
        return out;
    }
    public static Map<String,Object> storedWeapon(List<String> lore) {
        List<String> lines=lore.stream().map(MenuParser::clean).filter(l->l.startsWith("Stored Weapon: ")).toList();
        if(lines.size()!=1)return Map.of();
        Matcher m=Pattern.compile("^Stored Weapon: ([A-Za-z][A-Za-z '-]{0,95}?)(?: ([✪★]{1,15}))?$").matcher(lines.get(0));
        if(!m.matches()||Set.of("None","No Weapon").contains(m.group(1)))return Map.of();
        Map<String,Object> out=new LinkedHashMap<>();out.put("status","partial");out.put("displayName",m.group(1).trim());out.put("stars",m.group(2)==null?null:m.group(2).length());out.put("completeWeaponDetails",false);return out;
    }
    private static final Set<String> FACT_LABELS=Set.of("Level","Tier","Experience","HotM Experience","HotF Experience","Mithril Powder","Gemstone Powder","Glacite Powder","Tokens of the Mountain","Tokens of the Forest","Mining Speed","Mining Fortune","Foraging Fortune","Hunting Fortune","Charm Chance","Pristine","Breaking Power","Magical Power","Bonus Commissions Remaining","Daily Bonus Commissions","Charges","Remaining Charges","Copper","Bits","Motes","Whispers","Chocolate","Progress","Cost","Next Level Cost","Powder Cost","Fuel","Organic Matter","Item Tier","Collection","Health","Defense","Strength","Crit Damage","Crit Chance","Intelligence","Speed","Sea Creature Chance","Fishing Speed");
    public static List<Map<String,Object>> numericFacts(List<String> lines) {
        List<Map<String,Object>> out=new ArrayList<>();
        for(String raw:lines) {
            Matcher m=Pattern.compile("^([A-Za-z ]{1,40}): ([+]?[-]?[0-9][0-9,]*(?:[.][0-9]+)?)(?:/([0-9][0-9,]*(?:[.][0-9]+)?))?(?: (XP|coins|Powder|Whispers|Bits|Motes))?(%)?$").matcher(clean(raw).replaceFirst("^(Level|Tier) ([0-9])", "$1: $2"));
            if(!m.matches()||!FACT_LABELS.contains(m.group(1)))continue;
            double value=Double.parseDouble(m.group(2).replace(",",""));Double maximum=m.group(3)==null?null:Double.parseDouble(m.group(3).replace(",",""));
            if(!Double.isFinite(value)||Math.abs(value)>1e15||maximum!=null&&(!Double.isFinite(maximum)||maximum<0||maximum>1e15))continue;
            Map<String,Object> fact=new LinkedHashMap<>();fact.put("label",m.group(1));fact.put("value",value);fact.put("maximum",maximum);fact.put("unit",m.group(5)!=null?"%":m.group(4));out.add(fact);if(out.size()==64)break;
        }
        return out;
    }
    public static String kind(String title) {
        title=clean(title);
        if(HUNTAXE_TITLES.contains(title)||title.matches("Huntaxe(?: \\(Slot #\\d{1,2}\\))?"))return "huntaxe";
        if(Set.of("Heart of the Mountain","Heart of the Mountain (1/5)","Commissions","The Forge","Forge","Crystal Nucleus").contains(title))return "mining";
        if(Set.of("Heart of the Forest","Heart of the Forest (1/5)","Whispers").contains(title))return "foraging";
        if(Set.of("Hunting Box","Hunting Attributes","Hunting Traps","Fusion Machine").contains(title))return "hunting";
        if(Set.of("Experimentation Table","Experiments","Hoppity's Hunt","Chocolate Factory").contains(title))return "dailies";
        if(Set.of("Your SkyBlock Profile","Your Skills","SkyBlock Stats").contains(title))return "stats";
        if(Set.of("Pets","Pet Menu","Autopet Rules").contains(title))return "pets";
        if(Set.of("Collection","Collections").contains(title))return "collections";
        if(Set.of("Accessory Bag","Your Accessory Bag","Accessory Powers").contains(title))return "accessories";
        if(Set.of("SkyBlock Guide","Quest Log","Community Shop","Profile Upgrades").contains(title))return "quests";
        if(title.matches("Hunting Toolkit(?: \\(\\d+/\\d+\\))?"))return "toolkit";
        if(BANK_TITLES.contains(title))return "bank";
        if(title.equals("SkyBlock Menu")||title.equals("Cookie Buff")||title.matches("(?:\\(\\d+/\\d+\\) )?Active Effects"))return "buffs";
        if(title.matches("(?:Small |Medium |Large |Greater |Jumbo )?Backpack(?: \\(Slot #\\d{1,2}\\))?")||title.matches("Ender Chest(?: \\(\\d+/\\d+\\))?")||title.matches("Storage \\(\\d+/\\d+\\)"))return "storage";
        if(title.matches("[A-Za-z ]{1,40} Minion(?: [IVX]{1,8})?"))return "minion";
        if(GARDEN_TITLES.contains(title))return "garden";
        return null;
    }
    public static Map<String,Object> parse(String title, List<ItemView> slots) {
        String kind=kind(title);if(kind==null||slots.size()>54)return Map.of();
        Map<String,Object> out=new LinkedHashMap<>();
        if(kind.equals("huntaxe")) {
            for(ItemView item:slots)if(item.slot()==22&&item.id()!=null&&item.lore().stream().map(MenuParser::clean).anyMatch(l->l.matches("(?:a )?(?:COMMON|UNCOMMON|RARE|EPIC|LEGENDARY|MYTHIC|DIVINE|SPECIAL|VERY SPECIAL) (?:DUNGEON )?(?:SWORD|LONGSWORD|BOW|AXE)(?: a)?"))) { out.put("weapon",item.item());break; }
            // Missing/loading/placeholder slots cannot certify that no weapon is absorbed.
            return out;
        }
        if(kind.equals("bank")) {
            boolean personal=clean(title).startsWith("Personal"),shared=clean(title).startsWith("Co-op");
            for(ItemView item:slots)for(String raw:item.lore()) {String line=clean(raw);Matcher m=Pattern.compile("^(?:(Personal|Co-op) )?(?:Current [Bb]alance|Balance): ([0-9][0-9,]*(?:\\.[0-9]{1,2})?)(?: coins)?$").matcher(line);
                if(m.matches()){double n=Double.parseDouble(m.group(2).replace(",",""));if(!Double.isFinite(n)||n>1e15)continue;String prefix=m.group(1);String key="Personal".equals(prefix)||prefix==null&&personal?"personalBankCoins":"Co-op".equals(prefix)||prefix==null&&shared?"sharedProfileBankCoins":"bankCoinsUnclassified";out.put(key,n);}
            }
            return out;
        }
        if(kind.equals("buffs")) {
            boolean godFilter=slots.stream().anyMatch(i->clean(i.name()).equals("Filter")&&i.lore().stream().map(MenuParser::clean).anyMatch(l->l.equals("▶ God Potion Effects")));
            if(clean(title).matches("(?:\\(\\d+/\\d+\\) )?Active Effects")&&godFilter)
                for(ItemView i:slots)if(i.slot()==10)for(String raw:i.lore()){String line=clean(raw);if(line.startsWith("Remaining: ")){Long n=duration(line.substring(11));if(n!=null)out.put("godPotion",buff(n>0,n));}}

            for(ItemView item:slots) {
                String name=clean(item.name());
                if(name.equals("Booster Cookie"))for(String raw:item.lore()){String line=clean(raw);if(line.startsWith("Duration: ")){Long n=duration(line.substring(10));if(n!=null)out.put("cookie",buff(n>0,n));}else if(line.equals("Status: Not active!"))out.put("cookie",buff(false,0L));}
                if(name.equals("God Potion"))for(String raw:item.lore()){String line=clean(raw);if(line.startsWith("Remaining: ")){Long n=duration(line.substring(11));if(n!=null)out.put("godPotion",buff(n>0,n));}}
            }
            return out;
        }
        if(kind.equals("garden")&&clean(title).equals("Crop Diagnostics"))for(ItemView i:slots)if(i.slot()==20)for(String raw:i.lore()){String line=clean(raw);if(line.startsWith("Next Stage: ")){Long n=duration(line.substring(12));if(n!=null)out.put("greenhouseNextStageSecondsAtObservation",n);}}
        if(Set.of("mining","foraging","hunting","dailies","stats","pets","collections","accessories","quests").contains(kind)) {
            List<Map<String,Object>> facts=new ArrayList<>();
            for(ItemView i:slots) {
                String name=clean(i.name());if(!name.matches("[A-Za-z0-9 '():+./%✪★-]{1,96}"))continue;
                List<Map<String,Object>> fields=numericFacts(i.lore());
                if(!fields.isEmpty())facts.add(Map.of("slot",i.slot(),"displayName",name,"fields",fields));
            }
            out.put("facts",facts);out.put("observedSlots",slots.size());out.put("completeSystem",false);return out;
        }
        List<Map<String,Object>> items=new ArrayList<>();for(ItemView item:slots)if(item.id()!=null)items.add(item.item());
        out.put("items",items);out.put("observedSlots",slots.size());out.put("completeStorage",false);
        return out;
    }
}
