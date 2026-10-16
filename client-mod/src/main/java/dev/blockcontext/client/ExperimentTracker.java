package dev.blockcontext.client;
import java.util.*;
/** Net changes in owner-displayed charges, never inferred completions or daily totals. */
public final class ExperimentTracker {
    private long createdAt,decreases,increases,gaps,lastObservedAt;
    private int lastObserved=-1;
    private Integer anchor;
    private long anchorAt;
    private boolean restored;
    public void gap(){if(anchor!=null){gaps=Math.min(1000000,gaps+1);anchor=null;}}
    public void sample(Map<String,Object> menu,long now){
        if(createdAt==0)createdAt=now;
        Object v=menu.get("availableCharges");if(!(v instanceof Number n))return;
        double d=n.doubleValue();if(!Double.isFinite(d)||d<0||d>100||d!=Math.floor(d))return;
        int current=(int)d;
        if(anchor!=null&&now>=anchorAt&&now-anchorAt<=30*60*1000){
            if(current<anchor)decreases=Math.min(1000000,decreases+anchor-current);
            if(current>anchor)increases=Math.min(1000000,increases+current-anchor);
        }else if(anchor!=null)gap();
        anchor=current;anchorAt=now;lastObserved=current;lastObservedAt=now;
    }
    public Map<String,Object> tracking(boolean available){
        Map<String,Object> out=new LinkedHashMap<>();out.put("ledgerCreatedAt",createdAt);out.put("persistentObservedChargeDecreases",decreases);out.put("persistentObservedChargeIncreases",increases);out.put("observationGaps",gaps);out.put("restoredFromDisk",restored);out.put("persistenceAvailable",available);out.put("completeDailyCount",false);
        if(lastObserved>=0){out.put("lastObservedCharges",lastObserved);out.put("lastChargesObservedAt",lastObservedAt);}return out;
    }
    public long[] checkpoint(){return new long[]{createdAt,decreases,increases,gaps,lastObserved,lastObservedAt};}
    public static ExperimentTracker restore(long[] s,long now){
        if(s.length!=6||s[0]<=0||s[0]>now||s[1]<0||s[1]>1000000||s[2]<0||s[2]>1000000||s[3]<0||s[3]>1000000||s[4]<-1||s[4]>100||s[5]<0||s[5]>now||s[4]>=0&&s[5]<s[0]||s[4]<0&&s[5]!=0)throw new IllegalArgumentException("checkpoint");
        ExperimentTracker t=new ExperimentTracker();t.createdAt=s[0];t.decreases=s[1];t.increases=s[2];t.gaps=s[3];t.lastObserved=(int)s[4];t.lastObservedAt=s[5];t.restored=true;return t;
    }
}
