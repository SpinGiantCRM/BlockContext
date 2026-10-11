package dev.blockcontext.client;
import java.util.*;

/** Original passive UI state machine. Task completion counters are never claims.
 * Counts cover observed stable menu transitions in this run, not every claim today. */
public final class CommissionTracker {
    private Map<Integer,Map<String,Object>> previous=Map.of(),candidate=Map.of();
    private long candidateAt,lastSampleAt,claims,bonusClaims,lastAvailabilityAt,renewalAfter,renewalAt;
    private Boolean lastAvailability;
    public void gap(){previous=Map.of();candidate=Map.of();candidateAt=0;}
    @SuppressWarnings("unchecked")
    public Map<String,Object> sample(Map<String,Object> evidence,long now) {
        if(lastSampleAt>0&&now<lastSampleAt){lastAvailability=null;lastAvailabilityAt=0;renewalAfter=0;renewalAt=0;}
        if(lastSampleAt>0&&(now<lastSampleAt||now-lastSampleAt>5000))gap();
        lastSampleAt=now;
        Map<Integer,Map<String,Object>> rows=new LinkedHashMap<>();Set<String> names=new HashSet<>();
        for(var entry:(List<Map<String,Object>>)evidence.getOrDefault("entries",List.of())){
            String name=String.valueOf(entry.get("displayName"));
            if(!name.matches("Commission #[1-8](?: NEW)?"))continue;
            int slot=((Number)entry.get("slot")).intValue();
            if(!Boolean.TRUE.equals(entry.get("rewardPreviewValidated"))||rows.put(slot,entry)!=null||!names.add(name)){gap();return Map.of();}
        }
        // A fully received menu with at least the two initial slots. Placeholder,
        // mixed/partial reward updates and unrecognised formats cannot mean zero.
        if(rows.size()<2){gap();return Map.of();}
        if(!rows.equals(candidate)||now<candidateAt||now-candidateAt>5000){candidate=rows;candidateAt=now;return Map.of();}
        if(now-candidateAt<50)return Map.of();
        boolean available=rows.values().stream().anyMatch(r->Boolean.TRUE.equals(r.get("dailyBonusMarker")));
        if(Boolean.FALSE.equals(lastAvailability)&&available){renewalAfter=lastAvailabilityAt;renewalAt=now;}
        for(var e:rows.entrySet()){
            var old=previous.get(e.getKey());var next=e.getValue();
            if(old!=null&&Boolean.TRUE.equals(old.get("completedMarker"))&&Boolean.TRUE.equals(old.get("claimableMarker"))
               &&Boolean.FALSE.equals(next.get("completedMarker"))&&Boolean.FALSE.equals(next.get("claimableMarker"))&&zeroProgress(next)){
                claims=Math.min(1_000_000,claims+1);
                if(Boolean.TRUE.equals(old.get("dailyBonusMarker")))bonusClaims=Math.min(1_000_000,bonusClaims+1);
            }
        }
        previous=rows;lastAvailability=available;lastAvailabilityAt=now;
        Map<String,Object> out=new LinkedHashMap<>();out.put("dailyBonusAvailable",available);
        if(!available)out.put("remainingBonusCommissions",0);
        out.put("observedClaimTransitions",claims);out.put("observedBonusClaimTransitions",bonusClaims);
        out.put("claimCountScope","observed_menu_transitions_this_collector_run");
        out.put("availabilityEvidence","validated_current_reward_previews");
        if(renewalAt>0){out.put("bonusRenewalObservedAt",renewalAt);out.put("bonusRenewalAfter",renewalAfter);}
        return out;
    }
    @SuppressWarnings("unchecked") private static boolean zeroProgress(Map<String,Object> row){
        return ((List<Map<String,Object>>)row.getOrDefault("numericEvidence",List.of())).stream().anyMatch(n->"progress".equals(n.get("subject"))&&Boolean.TRUE.equals(n.get("percent"))&&n.get("numbers") instanceof List<?> values&&values.size()==1&&values.get(0) instanceof Number v&&v.doubleValue()==0);
    }
}
