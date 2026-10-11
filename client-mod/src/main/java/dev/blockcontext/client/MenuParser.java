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
    private static final Set<String> FACT_LABELS=Set.of("Level","Tier","Experience","HotM Experience","HotF Experience","Mithril Powder","Gemstone Powder","Glacite Powder","Tokens of the Mountain","Tokens of the Forest","Mining Speed","Mining Fortune","Foraging Fortune","Hunting Fortune","Charm Chance","Pristine","Breaking Power","Magical Power","Bonus Commissions Remaining","Daily Bonus Commissions","Charges","Remaining Charges","Copper","Bits","Motes","Whispers","Forest Whispers","Desert Whispers","Chocolate","Progress","Cost","Next Level Cost","Powder Cost","Fuel","Organic Matter","Item Tier","Collection","Health","Defense","Strength","Crit Damage","Crit Chance","Intelligence","Speed","Sea Creature Chance","Fishing Speed","Purse","Piggy","Bank","Mana","Farming Fortune","Mining Wisdom","Foraging Wisdom","Combat Wisdom","Fishing Wisdom","Enchanting Wisdom","Alchemy Wisdom","Taming Wisdom","Hunting Wisdom","Ability Damage","Bonus Attack Speed","True Defense","Vitality","Mending","Sweep","Pressure Resistance","Fossil Dust","Soulflow","Farming","Mining","Foraging","Combat","Fishing","Hunting","Enchanting","Alchemy","Taming","Carpentry","Runecrafting","Social");
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
    /** Only explicit owner UI counters; a reward marked Daily establishes bonus
     * availability, never its remaining count. No absence-based completion. */
    public static Map<String,Object> commissionCounters(List<String> lines) {
        Map<String,Object> out=new LinkedHashMap<>();Set<Integer> counts=new HashSet<>();Set<Long> resets=new HashSet<>();
        for(String raw:lines) {
            String line=clean(raw);
            Matcher m=Pattern.compile("^Bonus Commissions Remaining: ([0-9]{1,3})$").matcher(line);
            if(m.matches()){int n=Integer.parseInt(m.group(1));if(n<=100)counts.add(n);}
            if(line.startsWith("Daily Bonus Resets In: ")){Long n=duration(line.substring("Daily Bonus Resets In: ".length()));if(n!=null&&n<=86400)resets.add(n);}
        }
        if(counts.size()==1)out.put("remainingBonusCommissions",counts.iterator().next());
        if(resets.size()==1)out.put("nextResetSecondsAtObservation",resets.iterator().next());
        return out;
    }
    public static Map<String,Object> commissionMenu(List<ItemView> slots) {
        var out=commissionCounters(slots.stream().flatMap(i->i.lore().stream()).toList());
        boolean bonus=slots.stream().filter(i->i.slot()>=9&&i.slot()<18).flatMap(i->i.lore().stream()).map(MenuParser::clean).anyMatch(l->l.matches(".*\\(Daily(?: Bonus)?\\)$"));
        if(bonus&&Integer.valueOf(0).equals(out.get("remainingBonusCommissions")))out.remove("remainingBonusCommissions");
        if(bonus)out.put("dailyBonusAvailable",true);
        return out;
    }
    /** Bounded numeric menu evidence for validating claim/counter semantics.
     * No lore text, chat, click events or inferred successful claims are retained. */
    private static final Pattern COMMISSION_XP = Pattern.compile("^(?:- )?\\+([0-9][0-9,]{0,12}) HotM (?:XP|EXP|Experience)( \\(Daily(?: Bonus)?\\))?$", Pattern.CASE_INSENSITIVE);
    private static final Pattern COMMISSION_POWDER = Pattern.compile("^(?:- )?\\+?([0-9][0-9,]{0,12}) (?:Mithril|Gemstone|Glacite) Powder$");
    private static final Pattern COMMISSION_PROGRESS = Pattern.compile("^(?:[-_─━▬▏▎▍▌▋▊▉█ ]{1,100} )?([0-9]{1,3}(?:\\.[0-9]{1,6})?)%$");
    public static Map<String,Object> commissionEvidence(List<ItemView> slots) {
        List<Map<String,Object>> entries=new ArrayList<>();
        for(ItemView item:slots) {
            String name=clean(item.name());
            if(!((item.slot()>=9&&item.slot()<18)||name.equals("Commission Milestones")))continue;
            if(!name.matches("[A-Za-z0-9 '():+./%✪★#-]{1,96}"))continue;
            List<Map<String,Object>> numbers=new ArrayList<>();boolean claimable=false,completed=false,daily=false;int normalXp=0,dailyXp=0,powder=0,progress=0;
            int lineIndex=0;
            for(String raw:item.lore()) {
                String line=clean(raw);if(lineIndex>=64)break;
                claimable|=line.equals("Click to claim rewards!")||line.equals("Click to claim!");
                completed|=line.equals("COMPLETED")||line.equals("COMPLETED!");
                daily|=line.matches(".*\\(Daily(?: Bonus)?\\)$");
                Matcher xp=COMMISSION_XP.matcher(line);
                if(xp.matches()){long amount=Long.parseLong(xp.group(1).replace(",",""));if(amount>0&&amount<=100000){if(xp.group(2)==null)normalXp++;else dailyXp++;}}
                Matcher powderLine=COMMISSION_POWDER.matcher(line);
                if(powderLine.matches()){long amount=Long.parseLong(powderLine.group(1).replace(",",""));if(amount>0&&amount<=1e9)powder++;}
                Matcher progressLine=COMMISSION_PROGRESS.matcher(line);boolean progressValid=progressLine.matches()&&Double.parseDouble(progressLine.group(1))<=100;
                if(progressValid)progress++;
                String subject=progressValid?"progress":line.toLowerCase(Locale.ROOT).contains("hotm")?"hotm_xp":line.contains("Mining Experience")?"mining_xp":line.contains("Mithril Powder")?"mithril_powder":line.contains("Gemstone Powder")?"gemstone_powder":line.contains("Glacite Powder")?"glacite_powder":line.startsWith("Progress")?"progress":line.matches("(?:Total )?(?:Commissions Completed|Completed Commissions|Completed): .*")?"completed_counter":"unclassified";
                Matcher m=Pattern.compile("(?<![A-Za-z0-9])([0-9][0-9,]*(?:\\.[0-9]+)?)(?![A-Za-z0-9])").matcher(line);
                List<Double> values=new ArrayList<>();
                while(m.find()&&values.size()<4){double value=Double.parseDouble(m.group(1).replace(",",""));if(Double.isFinite(value)&&value<=1e9)values.add(value);}
                if(!values.isEmpty()&&numbers.size()<32)numbers.add(Map.of("line",lineIndex,"subject",subject,"numbers",values,"fraction",line.matches(".*[0-9]\\s*/\\s*[0-9].*"),"percent",line.contains("%")));
                lineIndex++;
            }
            boolean validated=normalXp==1&&powder==1&&(progress==1||completed&&claimable)&&dailyXp==(daily?1:0);
            if(!numbers.isEmpty()||claimable||completed||daily)entries.add(Map.of("slot",item.slot(),"displayName",name,"claimableMarker",claimable,"completedMarker",completed,"dailyBonusMarker",daily,"numericEvidence",numbers,"rewardPreviewValidated",validated));
        }
        return Map.of("entries",entries,"completeSystem",false,"claimsVerified",false);
    }
    public static List<Map<String,Object>> itemStats(List<String> lore) {
        List<Map<String,Object>> out=new ArrayList<>();
        for(String raw:lore) {
            String line=clean(raw).replaceFirst("^[^A-Za-z0-9+-]+", "");
            Matcher m=Pattern.compile("^(Damage|Health|Defense|Strength|Crit Damage|Crit Chance|Intelligence|Ability Damage|Bonus Attack Speed|Speed|Mining Speed|Mining Fortune|Farming Fortune|Foraging Fortune|Hunting Fortune|Pristine|Breaking Power|Sea Creature Chance|Fishing Speed|Sweep): ([+-]?[0-9][0-9,]*(?:[.][0-9]+)?)(%)?(?: .*)?$").matcher(line);
            if(!m.matches())continue;double value=Double.parseDouble(m.group(2).replace(",",""));if(!Double.isFinite(value)||Math.abs(value)>1e15)continue;
            out.add(Map.of("label",m.group(1),"value",value,"unit",m.group(3)==null?"stat":"%","valueKind","primary_displayed_item_stat"));
        }return out;
    }
    public static List<Map<String,Object>> hudFacts(String message) {
        List<Map<String,Object>> out=new ArrayList<>();String s=clean(message);
        for(var spec:List.of(new String[]{"Health","❤"},new String[]{"Mana","✎"},new String[]{"Defense","❈"})) {
            Matcher m=Pattern.compile("([0-9][0-9,]*(?:[.][0-9]+)?)(?:/([0-9][0-9,]*(?:[.][0-9]+)?))?"+spec[1]).matcher(s);
            if(m.find()){Map<String,Object> fact=new LinkedHashMap<>();fact.put("label",spec[0]);fact.put("value",Double.parseDouble(m.group(1).replace(",","")));fact.put("maximum",m.group(2)==null?null:Double.parseDouble(m.group(2).replace(",","")));fact.put("unit",null);out.add(fact);}
        }return out;
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
        if(Set.of("Museum", "Museum Browser", "Museum Rewards", "Your Museum").contains(title))return "museum";
        if(Set.of("Your Bazaar Orders", "Manage Orders", "Your Auctions", "Auction Bids").contains(title))return "orders";
        if(Set.of("Fast Travel", "Travel Scrolls", "Abiphone", "Abiphone Contacts").contains(title))return "travel";
        if(Set.of("Trophy Fish", "Trophy Fishing", "Fishing Equipment").contains(title))return "fishing";
        if(Set.of("Bestiary", "Bestiary Milestones").contains(title))return "bestiary";
        if(Set.of("Dungeon Stats", "Catacombs", "Dungeon Classes", "Dungeon Rewards").contains(title))return "dungeons";
        if(Set.of("Slayer", "Slayer Quest", "Slayer Rewards").contains(title))return "slayers";
        if(Set.of("SkyBlock Calendar", "Calendar and Events", "Mayor", "Election Room").contains(title))return "events";
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
        if(Set.of("mining","foraging","hunting","dailies","stats","pets","collections","accessories","quests","museum","orders","travel","fishing","bestiary","dungeons","slayers","events").contains(kind)) {
            List<Map<String,Object>> facts=new ArrayList<>();
            for(ItemView i:slots) {
                String name=clean(i.name());if(!name.matches("[A-Za-z0-9 '():+./%✪★-]{1,96}"))continue;
                List<Map<String,Object>> fields=numericFacts(i.lore());
                if(!fields.isEmpty())facts.add(Map.of("slot",i.slot(),"displayName",name,"fields",fields));
            }
            out.put("facts",facts);out.put("items",slots.stream().filter(i->i.id()!=null).map(ItemView::item).toList());out.put("observedSlots",slots.size());out.put("completeSystem",false);return out;
        }
        List<Map<String,Object>> items=new ArrayList<>();for(ItemView item:slots)if(item.id()!=null)items.add(item.item());
        out.put("items",items);out.put("observedSlots",slots.size());out.put("completeStorage",false);
        return out;
    }
}
