package dev.blockcontext.client;
import java.util.*;
public final class MenuParserTest {
    public static void main(String[] args) {
        assert MenuParser.duration("4d 2h 3m 4s")==352984;
        assert MenuParser.duration("12 Hours")==43200;
        assert MenuParser.duration("105:01:34")==378094;
        for(String s:List.of("-1h","1h send secrets","1h 2h","105:61:34","forever",""))assert MenuParser.duration(s)==null:s;
        assert "Strawberry".equals(MenuParser.profile(List.of("§eProfile: §aStrawberry")));
        assert "Apple".equals(MenuParser.profile(List.of("Profile: Apple ♲")));
        assert MenuParser.profile(List.of("Profile: Apple","Profile: Strawberry"))==null;
        assert MenuParser.profile(List.of("Player: Profile: Strawberry"))==null;
        assert MenuParser.tabBuffs(List.of("Cookie Buff","Not active!")).get("cookie").get("active").equals(false);
        assert MenuParser.tabBuffs(List.of("God Potion: 4d")).get("godPotion").get("remainingSecondsAtObservation").equals(345600L);
        assert MenuParser.tabBuffs(List.of("Nothing here")).isEmpty();
        assert MenuParser.tabBuffs(List.of("You have a God Potion active! 12 Hours")).get("godPotion").get("remainingSecondsAtObservation").equals(43200L);
        assert MenuParser.parse("Huntaxe",List.of(new MenuParser.ItemView(22,"STONE","Placeholder",List.of(),Map.of()))).isEmpty();
        var filter=new MenuParser.ItemView(53,null,"Filter",List.of("▶ God Potion Effects"),Map.of());
        var effect=new MenuParser.ItemView(10,null,"Strength",List.of("Remaining: 105:01:34"),Map.of());
        assert MenuParser.parse("Active Effects",List.of(filter,effect)).containsKey("godPotion");
        assert !MenuParser.parse("Active Effects",List.of(effect)).containsKey("godPotion");
        assert MenuParser.kind("Huntaxe")!=null;
        assert "huntaxe".equals(MenuParser.kind("§8Prime Huntaxe - Nex Titanum"));
        assert MenuParser.kind("Trade - Prime Huntaxe - Nex Titanum") == null;
        assert MenuParser.kind("Prime Huntaxe - Nex Titanum trade") == null;
        assert MenuParser.kind("Trade") == null;
        assert MenuParser.kind("Banker Broadjaw") == null;
        assert MenuParser.parse("Huntaxe",List.of()).isEmpty();
        Map<String,Object> weapon=Map.of("id","HYPER_CLEAVER","reforge","fabled","enchantments",Map.of("sharpness",6));
        assert MenuParser.parse("Huntaxe",List.of(new MenuParser.ItemView(22,"HYPER_CLEAVER","Fabled Hyper Cleaver",List.of("§5EPIC DUNGEON SWORD"),weapon))).get("weapon").equals(weapon);
        assert MenuParser.parse("Prime Huntaxe - Nex Titanum",List.of(new MenuParser.ItemView(22,"HYPER_CLEAVER","Fabled Hyper Cleaver",List.of("§5EPIC DUNGEON SWORD", "Click to take!"),weapon))).get("weapon").equals(weapon);
        for(String title:List.of("Worn Huntaxe - Genesis", "Sharpened Huntaxe - Dominus", "Reinforced Huntaxe - Cursus", "Savage Huntaxe - Praedator", "Prime Huntaxe - Nex Titanum")) {
            assert "huntaxe".equals(MenuParser.kind(title));
            assert MenuParser.parse(title,List.of(new MenuParser.ItemView(22,"HYPER_CLEAVER","Fabled Hyper Cleaver",List.of("§5EPIC DUNGEON SWORD"),weapon))).get("weapon").equals(weapon):title;
            assert MenuParser.kind("Trade: "+title)==null;
            assert MenuParser.parse(title,List.of(new MenuParser.ItemView(23,"HYPER_CLEAVER","Fabled Hyper Cleaver",List.of("§5EPIC DUNGEON SWORD"),weapon))).isEmpty();
        }
        assert MenuParser.kind("Worn Huntaxe - Nex Titanum")==null;
        assert MenuParser.parse("Prime Huntaxe - Nex Titanum",List.of(new MenuParser.ItemView(22,"STONE","Placeholder",List.of("Click to take!"),Map.of()))).isEmpty();
        assert MenuParser.parse("Huntaxe",List.of(new MenuParser.ItemView(23,"HYPER_CLEAVER","Fabled Hyper Cleaver",List.of("§5EPIC DUNGEON SWORD"),weapon))).isEmpty();
        var balance=new MenuParser.ItemView(1,null,"Balance",List.of("Current balance: 12,345.67 coins"),Map.of());
        assert MenuParser.parse("Personal Bank Account",List.of(balance)).get("personalBankCoins").equals(12345.67);
        assert MenuParser.parse("Co-op Bank Account",List.of(balance)).get("sharedProfileBankCoins").equals(12345.67);
        assert MenuParser.parse("Bank",List.of(balance)).containsKey("bankCoinsUnclassified");
        assert MenuParser.parse("Bank",List.of(new MenuParser.ItemView(1,null,"Balance",List.of("Someone says: Current Balance: 123 coins"),Map.of()))).isEmpty();
        assert MenuParser.parse("Jumbo Backpack",List.of()).get("completeStorage").equals(false);
        assert MenuParser.parse("Crop Diagnostics",List.of(new MenuParser.ItemView(20,null,"Growth",List.of("Next Stage: 1h 40m 20s"),Map.of()))).get("greenhouseNextStageSecondsAtObservation").equals(6020L);
        assert !MenuParser.parse("Crop Diagnostics",List.of(new MenuParser.ItemView(19,null,"Growth",List.of("Next Stage: 1h 40m 20s"),Map.of()))).containsKey("greenhouseNextStageSecondsAtObservation");
        assert MenuParser.storedWeapon(List.of("§7Stored Weapon: §5Fabled Hyper Cleaver §6✪✪✪✪✪")).get("stars").equals(5);
        assert MenuParser.storedWeapon(List.of("Stored Weapon: None")).isEmpty();
        assert MenuParser.storedWeapon(List.of("Stored Weapon: Fabled Hyper Cleaver", "Stored Weapon: None")).isEmpty();
        assert MenuParser.numericFacts(List.of("Level 20/50", "Mithril Powder: 1,343", "Progress: 100/300 XP")).size()==3;
        assert MenuParser.numericFacts(List.of("Coins: 20", "Someone: 34", "Mining Speed: NaN", "Mining Speed: 12 do things")).isEmpty();
        assert MenuParser.parse("Heart of the Mountain",List.of(new MenuParser.ItemView(1,null,"Mining Speed",List.of("Level 20/50"),Map.of()))).get("completeSystem").equals(false);
        assert MenuParser.parse("Huntaxe",List.of(new MenuParser.ItemView(22,"HYPER_CLEAVER","Fabled Hyper Cleaver",List.of("§ka §5EPIC DUNGEON SWORD §ka"),weapon))).get("weapon").equals(weapon);
        assert MenuParser.numericFacts(List.of("Forest Whispers: 2,160", "Desert Whispers: 0")).size()==2;
        assert MenuParser.commissionCounters(List.of("Bonus Commissions Remaining: 0")).get("remainingBonusCommissions").equals(0);
        assert MenuParser.commissionCounters(List.of("Bonus Commissions Remaining: 2", "Bonus Commissions Remaining: 1")).isEmpty();
        assert MenuParser.commissionCounters(List.of("Someone: Bonus Commissions Remaining: 0", "Bonus Commissions Remaining: -1", "Bonus Commissions Remaining: 1.5")).isEmpty();
        assert MenuParser.commissionCounters(List.of("Daily Bonus Resets In: 1h 20m")).get("nextResetSecondsAtObservation").equals(4800L);
        var daily=new MenuParser.ItemView(9,null,"Commission",List.of("+900 HotM XP (Daily Bonus)"),Map.of());
        assert MenuParser.commissionMenu(List.of(daily)).get("dailyBonusAvailable").equals(true);
        assert !MenuParser.commissionMenu(List.of(daily)).containsKey("remainingBonusCommissions");
        assert MenuParser.commissionMenu(List.of(new MenuParser.ItemView(8,null,"Other",daily.lore(),Map.of()))).isEmpty();
        assert MenuParser.commissionMenu(List.of()).isEmpty();
        var evidence=MenuParser.commissionEvidence(List.of(new MenuParser.ItemView(9,null,"Commission #1",List.of("Progress: 25/100", "+900 HotM XP (Daily Bonus)", "Click to claim rewards!", "Private arbitrary text"),Map.of())));
        assert evidence.get("claimsVerified").equals(false);
        assert evidence.toString().contains("hotm_xp");assert evidence.toString().contains("progress");assert evidence.toString().contains("claimableMarker=true");
        assert !evidence.toString().contains("Private arbitrary text");
        assert MenuParser.commissionEvidence(List.of(new MenuParser.ItemView(8,null,"Other",List.of("Progress: 1/100"),Map.of()))).get("entries").equals(List.of());
        assert MenuParser.commissionEvidence(List.of(new MenuParser.ItemView(27,null,"Commission Milestones",List.of("Completed: 48", "Progress: 48/100"),Map.of()))).toString().contains("completed_counter");
        assert !MenuParser.commissionEvidence(List.of(new MenuParser.ItemView(9,null,"Commission",List.of("uuid aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "send a secret"),Map.of()))).toString().contains("aaaaaa");
        testCommissionTracking();
        var safe=ItemFacts.project(Map.of("uuid","b".repeat(32),"owner","Secret","petInfo",Map.of("type","GOLDEN_DRAGON","exp",12345),"new_item_counter",12));
        assert !safe.toString().contains("Secret");assert !safe.toString().contains("bbbbbbbb");assert safe.toString().contains("new_item_counter");
        assert MenuParser.itemStats(List.of("Damage: +212.5 (+20) (+594.75)")).get(0).get("value").equals(212.5);
        assert MenuParser.hudFacts("1,234/2,345❤   562❈ Defense 400/500✎ Mana").size()==3;
        assert MenuParser.kind("Your Bazaar Orders").equals("orders");assert MenuParser.kind("Trade")==null;
        var changes=ObservationChanges.compare("inventory",Map.of("coins",10,"uuid","a".repeat(32)),Map.of("coins",20,"uuid","b".repeat(32)),100,200);
        assert changes.size()==1;assert changes.get(0).get("path").equals("inventory.coins");assert changes.get(0).get("previousObservedAt").equals(100L);assert !changes.toString().contains("bbbb");
        assert ObservationChanges.compare("inventory",Map.of(),Map.of("coins",0),100,200).isEmpty();
        System.out.println("MenuParser: duration, profile binding, buffs, menu isolation, Huntaxe slot, bank separation and partial storage checks passed");
    }
    private static Map<String,Object> commissionSnapshot(boolean daily,boolean ready,int milestone){
        List<MenuParser.ItemView> items=new ArrayList<>();
        for(int i=0;i<2;i++){
            List<String> lore=new ArrayList<>(List.of("Rewards"));if(daily)lore.add("§7- §5+900 HOTM Exp §7(Daily Bonus)");
            lore.add("§7- §5+410 HOTM Exp");lore.add("§7- §2500 Mithril Powder");lore.add("§7- §3+7.5k Mining Exp");lore.add("Progress");
            if(i==1&&ready){lore.add("COMPLETED");lore.add("Click to claim rewards!");}else lore.add("§f§m--------------------§r §e0%");
            items.add(new MenuParser.ItemView(i==0?11:15,null,"Commission #"+(i+1)+" NEW",lore,Map.of()));
        }
        items.add(new MenuParser.ItemView(30,null,"Commission Milestones",List.of("Progress: "+milestone+"%",milestone+"/100"),Map.of()));
        return MenuParser.commissionEvidence(items);
    }
    private static void testCommissionTracking(){
        for(String bar:List.of("0%","-------------------- 0%","██████████ 5.25%","                    100%")) {
            var proof=MenuParser.commissionEvidence(List.of(new MenuParser.ItemView(11,null,"Commission #1",List.of("- +410 HOTM Exp","- 500 Mithril Powder",bar),Map.of())));
            assert proof.toString().contains("rewardPreviewValidated=true"):bar;
        }
        for(List<String> lore:List.of(
            List.of("- +410 HOTM Exp","- 500 Mithril Powder","-------------------- 101%"),
            List.of("- +410 HOTM Exp","- 500 Mithril Powder","Other stat: 0%"),
            List.of("- +410 HOTM Exp","- 500 Mithril Powder","0%","- +410 HOTM Exp"),
            List.of("- +410 HOTM Exp","- 500 Mithril Powder","0%","0%"),
            List.of("- +410 HOTM Exp","- 0 Mithril Powder","0%"),
            List.of("- +410 HOTM Exp extra text","- 500 Mithril Powder","0%"),
            List.of("- +410 HOTM Exp","- 500 Mithril Powder","0%","Private text (Daily Bonus)")
        )) {
            var proof=MenuParser.commissionEvidence(List.of(new MenuParser.ItemView(11,null,"Commission #1",lore,Map.of())));
            assert proof.toString().contains("rewardPreviewValidated=false"):lore;
        }
        CommissionTracker tracker=new CommissionTracker();long t=10000;
        var active=commissionSnapshot(true,false,44);assert tracker.sample(active,t).isEmpty();
        var start=tracker.sample(active,t+50);assert start.get("dailyBonusAvailable").equals(true);assert !start.containsKey("remainingBonusCommissions");
        var ready=commissionSnapshot(true,true,45);tracker.sample(ready,t+100);
        assert tracker.sample(ready,t+150).get("observedClaimTransitions").equals(0L):"task completion is not a claim";
        var claimed=commissionSnapshot(false,false,45);tracker.sample(claimed,t+200);var end=tracker.sample(claimed,t+250);
        assert end.get("remainingBonusCommissions").equals(0);assert end.get("observedClaimTransitions").equals(1L);assert end.get("observedBonusClaimTransitions").equals(1L);
        assert tracker.sample(claimed,t+300).get("observedClaimTransitions").equals(1L):"stable repeated previews must not duplicate a claim";
        tracker.sample(active,t+350);var renewed=tracker.sample(active,t+400);assert renewed.get("bonusRenewalAfter").equals(t+300);assert renewed.get("bonusRenewalObservedAt").equals(t+400);assert !renewed.containsKey("remainingBonusCommissions"):"renewal does not establish four remaining";
        tracker.sample(ready,t+450);tracker.sample(ready,t+500);tracker.gap();tracker.sample(claimed,t+550);
        assert tracker.sample(claimed,t+600).get("observedClaimTransitions").equals(1L):"menu/disconnect gaps cannot prove claims";
        tracker.sample(ready,t+650);tracker.sample(ready,t+700);tracker.sample(claimed,t+10000);
        assert tracker.sample(claimed,t+10050).get("observedClaimTransitions").equals(1L):"long sampling gaps cannot prove claims";
        CommissionTracker other=new CommissionTracker();assert other.sample(claimed,t).isEmpty();assert other.sample(claimed,t+50).get("observedClaimTransitions").equals(0L);
        assert tracker.sample(MenuParser.commissionEvidence(List.of()),t+11000).isEmpty():"empty menu must not mean exhausted";
        var partial=commissionSnapshot(false,false,45);@SuppressWarnings("unchecked") var rows=(List<Map<String,Object>>)partial.get("entries");
        List<Map<String,Object>> broken=new ArrayList<>(rows);Map<String,Object> row=new LinkedHashMap<>(broken.get(0));row.put("rewardPreviewValidated",false);broken.set(0,row);
        assert tracker.sample(Map.of("entries",broken),t+11050).isEmpty():"incomplete rewards must not mean zero";
        System.out.println("CommissionTracker: completion/claim separation, exhaustion, renewal, duplicate, gap and profile isolation checks passed");
    }
}
