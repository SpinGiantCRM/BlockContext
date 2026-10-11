package dev.blockcontext.client;
import java.util.*;
import java.util.regex.*;
/** Bounded, typed item attributes. No raw NBT, identities, owners or arbitrary lore. */
public final class ItemFacts {
    private static final Pattern PRIVATE=Pattern.compile("(?:^|_)(?:uuid|uniqueid|player_id|player_uuid|owner|owners|donator|originator|email|ip|token|secret|password|session|chat|message|history)(?:$|_)",Pattern.CASE_INSENSITIVE);
    private static final Pattern UUID=Pattern.compile("(?:[a-f0-9]{32}|[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12})",Pattern.CASE_INSENSITIVE);
    public static boolean safeKey(String key){return key.matches("[A-Za-z0-9_:-]{1,64}")&&!PRIVATE.matcher(key).find()&&!UUID.matcher(key).matches();}
    public static Map<String,Object> project(Map<String,Object> input) {
        List<Map<String,Object>> fields=new ArrayList<>();int[] omitted={0},budget={512};boolean[] truncated={false};
        visit(input,"",0,fields,omitted,budget,truncated);
        return Map.of("fields",fields,"omitted",omitted[0],"truncated",truncated[0],"complete",false);
    }
    private static void visit(Object v,String path,int depth,List<Map<String,Object>> fields,int[] omitted,int[] budget,boolean[] truncated){
        if(--budget[0]<0||depth>8||fields.size()>=128){truncated[0]=true;return;}
        if(v instanceof Map<?,?> map){for(var e:map.entrySet()){String k=String.valueOf(e.getKey());if(safeKey(k))visit(e.getValue(),path.isEmpty()?k:path+"."+k,depth+1,fields,omitted,budget,truncated);else omitted[0]++;}return;}
        if(v instanceof List<?> list){for(int i=0;i<Math.min(64,list.size());i++)visit(list.get(i),path+"."+i,depth+1,fields,omitted,budget,truncated);if(list.size()>64)truncated[0]=true;return;}
        if(path.isEmpty()||path.length()>128){omitted[0]++;return;}
        if(v instanceof Number n){if(!Double.isFinite(n.doubleValue())||Math.abs(n.doubleValue())>1e15){omitted[0]++;return;}}
        else if(v instanceof String s){if(!s.matches("[A-Za-z0-9_ :.+/%,()'★✪-]{0,96}")||UUID.matcher(s).matches()){omitted[0]++;return;}}
        else if(!(v instanceof Boolean)||v==null){omitted[0]++;return;}
        fields.add(Map.of("path",path,"value",v,"semanticStatus","reported_attribute"));
    }
}
