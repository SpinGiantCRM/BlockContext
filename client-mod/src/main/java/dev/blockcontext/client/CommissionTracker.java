package dev.blockcontext.client;
import java.util.*;

/** Original passive UI state machine. Task completion counters are never claims.
 * Counts cover observed stable menu transitions in a persisted ledger, not every claim today. */
public final class CommissionTracker {
    private Map<Integer,Map<String,Object>> previous=Map.of(),candidate=Map.of();
    private long candidateAt,lastSampleAt,claims,bonusClaims,lastAvailabilityAt,renewalAfter,renewalAt;
    private Boolean lastAvailability;
    private long createdAt,referenceDay,dayBonusClaims,gaps,lastRemainingAt,lastStableAt,partialSince;
    private int lastRemaining=-1,exactRemaining=-1;
    private boolean restored;
    public static long referenceDay(long now){return Math.floorDiv(now-18_000_000L,86_400_000L)*86_400_000L+18_000_000L;}
    public void gap(){if(!previous.isEmpty())gaps=Math.min(1_000_000,gaps+1);previous=Map.of();candidate=Map.of();candidateAt=0;exactRemaining=-1;partialSince=0;lastStableAt=0;}
    private void partial(long now){candidate=Map.of();candidateAt=0;exactRemaining=-1;if(partialSince==0)partialSince=now;if(lastStableAt==0||now<lastStableAt||now-lastStableAt>2000)gap();}
    public Map<String,Object> persistentState(){
        Map<String,Object> out=new LinkedHashMap<>();out.put("schemaVersion",1);out.put("createdAt",createdAt);out.put("claims",claims);out.put("bonusClaims",bonusClaims);out.put("lastAvailability",lastAvailability);out.put("lastAvailabilityAt",lastAvailabilityAt);out.put("renewalAfter",renewalAfter);out.put("renewalAt",renewalAt);out.put("referenceDay",referenceDay);out.put("dayBonusClaims",dayBonusClaims);out.put("gaps",gaps);out.put("lastRemaining",lastRemaining);out.put("lastRemainingAt",lastRemainingAt);return out;
    }
    public static CommissionTracker restore(Map<String,Object> state,long now){
        if(integer(state,"schemaVersion",1,1)!=1)throw new IllegalArgumentException("schema");
        CommissionTracker t=new CommissionTracker();t.createdAt=integer(state,"createdAt",0,now);t.claims=integer(state,"claims",0,1_000_000);t.bonusClaims=integer(state,"bonusClaims",0,t.claims);
        Object available=state.get("lastAvailability");if(available!=null&&!(available instanceof Boolean))throw new IllegalArgumentException("availability");t.lastAvailability=(Boolean)available;
        t.lastAvailabilityAt=integer(state,"lastAvailabilityAt",0,now);t.renewalAt=integer(state,"renewalAt",0,now);t.renewalAfter=integer(state,"renewalAfter",0,t.renewalAt);
        t.referenceDay=integer(state,"referenceDay",0,now);if(t.referenceDay!=0&&referenceDay(t.referenceDay)!=t.referenceDay)throw new IllegalArgumentException("day");
        t.dayBonusClaims=integer(state,"dayBonusClaims",0,t.bonusClaims);t.gaps=Math.min(1_000_000,integer(state,"gaps",0,1_000_000)+1);t.lastRemaining=(int)integer(state,"lastRemaining",-1,100);t.lastRemainingAt=integer(state,"lastRemainingAt",0,now);t.restored=true;return t;
    }
    private static long integer(Map<String,Object> state,String key,long min,long max){Object value=state.get(key);if(!(value instanceof Number n)||!Double.isFinite(n.doubleValue())||n.doubleValue()!=n.longValue()||n.longValue()<min||n.longValue()>max)throw new IllegalArgumentException("invalid state");return n.longValue();}
    public Map<String,Object> tracking(long now){
        if(createdAt==0)createdAt=now;
        long day=referenceDay(now);Map<String,Object> out=new LinkedHashMap<>();out.put("ledgerCreatedAt",createdAt);out.put("persistentObservedClaims",claims);out.put("persistentObservedBonusClaims",bonusClaims);out.put("observationGaps",gaps);out.put("restoredFromDisk",restored);out.put("completeDailyCount",false);out.put("referenceDayStart",day);out.put("observedBonusClaimsInReferenceDay",day==referenceDay?dayBonusClaims:0L);out.put("referenceResetUtcHour",5);out.put("referenceResetServerVerified",false);out.put("expectedNextResetAt",day+86_400_000L);
        if(lastRemaining>=0){out.put("lastObservedRemaining",lastRemaining);out.put("lastRemainingObservedAt",lastRemainingAt);out.put("referenceResetPassedSinceObservation",referenceDay(lastRemainingAt)!=day);}
        if(renewalAt>0){out.put("bonusRenewalAfter",renewalAfter);out.put("bonusRenewalObservedAt",renewalAt);long boundary=referenceDay(renewalAt);out.put("referenceResetFallsInObservedWindow",renewalAfter<boundary&&boundary<=renewalAt);out.put("resetObservationWindowMilliseconds",renewalAt-renewalAfter);}
        return out;
    }
    public Map<String,Object> sample(Map<String,Object> evidence,long now){return sample(evidence,now,Map.of());}
    @SuppressWarnings("unchecked")
    public Map<String,Object> sample(Map<String,Object> evidence,long now,Map<String,Object> explicit) {
        if(createdAt==0)createdAt=now;
        long day=referenceDay(now);if(day!=referenceDay){referenceDay=day;dayBonusClaims=0;exactRemaining=-1;}
        if(lastSampleAt>0&&now<lastSampleAt){lastAvailability=null;lastAvailabilityAt=0;renewalAfter=0;renewalAt=0;}
        if(lastSampleAt>0&&(now<lastSampleAt||now-lastSampleAt>5000))gap();
        lastSampleAt=now;
        if(partialSince>0&&(now<lastStableAt||now-lastStableAt>2000))gap();
        Map<Integer,Map<String,Object>> rows=new LinkedHashMap<>();Set<String> names=new HashSet<>();
        for(var entry:(List<Map<String,Object>>)evidence.getOrDefault("entries",List.of())){
            String name=String.valueOf(entry.get("displayName"));
            if(!name.matches("Commission #[1-8](?: NEW)?"))continue;
            int slot=((Number)entry.get("slot")).intValue();
            if(!Boolean.TRUE.equals(entry.get("rewardPreviewValidated"))||rows.put(slot,entry)!=null||!names.add(name.replaceFirst(" NEW$",""))){partial(now);return Map.of();}
        }
        // A fully received menu with at least the two initial slots. Placeholder,
        // mixed/partial reward updates and unrecognised formats cannot mean zero.
        if(rows.size()<2){partial(now);return Map.of();}
        if(!rows.equals(candidate)||now<candidateAt||now-candidateAt>5000){candidate=rows;candidateAt=now;return Map.of();}
        if(now-candidateAt<50)return Map.of();
        boolean available=rows.values().stream().anyMatch(r->Boolean.TRUE.equals(r.get("dailyBonusMarker")));
        if(Boolean.FALSE.equals(lastAvailability)&&available){renewalAfter=lastAvailabilityAt;renewalAt=now;}
        for(var e:rows.entrySet()){
            var old=previous.get(e.getKey());var next=e.getValue();
            if(old!=null&&String.valueOf(old.get("displayName")).replaceFirst(" NEW$","").equals(String.valueOf(next.get("displayName")).replaceFirst(" NEW$",""))&&Boolean.TRUE.equals(old.get("completedMarker"))&&Boolean.TRUE.equals(old.get("claimableMarker"))
               &&Boolean.FALSE.equals(next.get("completedMarker"))&&Boolean.FALSE.equals(next.get("claimableMarker"))&&zeroProgress(next)){
                claims=Math.min(1_000_000,claims+1);
                if(Boolean.TRUE.equals(old.get("dailyBonusMarker"))){bonusClaims=Math.min(1_000_000,bonusClaims+1);dayBonusClaims=Math.min(bonusClaims,dayBonusClaims+1);if(exactRemaining>0)exactRemaining--;}
            }
        }
        previous=rows;lastAvailability=available;lastAvailabilityAt=now;lastStableAt=now;partialSince=0;
        Map<String,Object> out=new LinkedHashMap<>();out.put("dailyBonusAvailable",available);
        Object counter=explicit.get("remainingBonusCommissions");String countEvidence="explicit_counter_minus_observed_claims";
        if(counter instanceof Number n&&n.doubleValue()==n.intValue()&&n.intValue()>=0&&n.intValue()<=100&&(n.intValue()>0)==available){exactRemaining=n.intValue();countEvidence="explicit_current_counter";}
        if(!available){exactRemaining=0;countEvidence="validated_reward_exhaustion";}else if(exactRemaining==0)exactRemaining=-1;
        if(exactRemaining>=0){out.put("remainingBonusCommissions",exactRemaining);out.put("remainingCountEvidence",countEvidence);lastRemaining=exactRemaining;lastRemainingAt=now;}
        out.put("observedClaimTransitions",claims);out.put("observedBonusClaimTransitions",bonusClaims);
        out.put("claimCountScope","observed_menu_transitions_since_ledger_creation");
        out.put("availabilityEvidence","validated_current_reward_previews");
        if(renewalAt>0){out.put("bonusRenewalObservedAt",renewalAt);out.put("bonusRenewalAfter",renewalAfter);}
        return out;
    }
    @SuppressWarnings("unchecked") private static boolean zeroProgress(Map<String,Object> row){
        return ((List<Map<String,Object>>)row.getOrDefault("numericEvidence",List.of())).stream().anyMatch(n->"progress".equals(n.get("subject"))&&Boolean.TRUE.equals(n.get("percent"))&&n.get("numbers") instanceof List<?> values&&values.size()==1&&values.get(0) instanceof Number v&&v.doubleValue()==0);
    }
}
