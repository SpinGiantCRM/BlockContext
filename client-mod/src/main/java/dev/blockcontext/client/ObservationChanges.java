package dev.blockcontext.client;
import java.util.*;
/** Bounded changes in owner-visible observations; never classifies sales, ownership loss or completion. */
public final class ObservationChanges {
 private ObservationChanges(){}
 public static List<Map<String,Object>> compare(String source,Map<String,Object> before,Map<String,Object> after,long previousAt,long observedAt){
  Map<String,Object> a=new LinkedHashMap<>(),b=new LinkedHashMap<>();flatten(before,"",a,0);flatten(after,"",b,0);List<Map<String,Object>> out=new ArrayList<>();
  for(var e:b.entrySet())if(out.size()<32&&a.containsKey(e.getKey())&&!Objects.equals(a.get(e.getKey()),e.getValue())){
   Map<String,Object> row=new LinkedHashMap<>();row.put("path",source+"."+e.getKey());row.put("previousValue",a.get(e.getKey()));row.put("value",e.getValue());row.put("previousObservedAt",previousAt);row.put("observedAt",observedAt);row.put("kind","observed_value_change");out.add(row);
  }return out;
 }
 private static void flatten(Object value,String path,Map<String,Object> out,int depth){
  if(depth>10||out.size()>=512||path.length()>128)return;
  if(value instanceof Number n){double d=n.doubleValue();if(Double.isFinite(d)&&Math.abs(d)<=1e15)out.put(path,d);}
  else if(value instanceof Boolean||value instanceof String s&&s.matches("[A-Z0-9_:.-]{1,96}"))out.put(path,value);
  else if(value instanceof Map<?,?> map){for(var e:map.entrySet())if(e.getKey() instanceof String key&&ItemFacts.safeKey(key))flatten(e.getValue(),path.isEmpty()?key:path+"."+key,out,depth+1);}
  else if(value instanceof List<?> list)for(int i=0;i<Math.min(128,list.size());i++)flatten(list.get(i),path+"."+i,out,depth+1);
 }
}
