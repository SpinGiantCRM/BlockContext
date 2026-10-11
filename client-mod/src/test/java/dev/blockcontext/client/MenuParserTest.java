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
}
