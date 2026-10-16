//! Experimentation evidence: API counters, explicit menu state and partial charge accounting.
use crate::types::*;
use serde_json::{Value, json};
fn count(v: &Value) -> Option<u64> {
    v.unsigned().filter(|n| *n <= 9007199254740991)
}
pub fn api_usage(raw: &Value) -> Value {
    let games = [("pairings", "Superpairs"), ("simon", "Chronomatron"), ("numbers", "Ultrasequencer")]
        .into_iter().map(|(id,name)| {
            let game=&raw[id];
            let counters=|prefix:&str| obj(game).into_iter().filter_map(|(k,v)| {
                let suffix=k.strip_prefix(prefix)?;
                let tier=suffix.parse::<u8>().ok()?;
                if tier.to_string()!=suffix{return None;}
                (tier<=15).then_some(())?;
                Some((k,json!(count(&v)?)))
            }).collect::<serde_json::Map<_,_>>();
            let claims=counters("claims_");let attempts=counters("attempts_");
            let subtotal=(!claims.is_empty()).then(|| claims.values().filter_map(count).try_fold(0_u64,|sum,n|sum.checked_add(n).filter(|n|*n<=9007199254740991))).flatten();
            json!({"id":id,"name":name,"status":if game.is_object(){"partial"}else{"unavailable"},"reportedClaimsByTier":claims,"reportedAttemptsByTier":attempts,"reportedClaimCountSubtotal":subtotal,"allTiersExposed":false,"claimedFlag":game["claimed"].as_bool(),"lastAttemptReported":count(&game["last_attempt"]),"lastClaimedReported":count(&game["last_claimed"]),"source":format!("member.experimentation.{id}"),"dailyUseCount":null})
        }).collect::<Vec<_>>();
    json!({"games":games,"chargeTrackingTimestampReported":count(&raw["charge_track_timestamp"]),"paidResetsReported":count(&raw["claims_resets"]),"paidResetsTimestampReported":count(&raw["claims_resets_timestamp"]),"currentCharges":null,"usesToday":null,"exactResetTimeKnown":false,"reasonCode":"API_counters_and_flags_do_not_certify_current_charges_or_daily_uses"})
}
pub fn reconcile(activity: &Value, client: &Value) -> Value {
    let row = at(client, "observations.experimentTable");
    let values = &row["values"];
    let current = row["currentStateVerified"] == true;
    let remaining = count(&values["availableCharges"]).filter(|n| *n <= 100);
    let ledger = at(client, "observations.experimentTracking");
    let api = api_usage(at(activity, "evidence.value"));
    let recharge = count(&values["nextChargeSecondsAtObservation"]);
    let reset = count(&values["paidResetSecondsAtObservation"]);
    merge(
        activity.clone(),
        json!({"status":if current&&remaining.is_some(){"known"}else if !row.is_null() || !ledger.is_null() || at(activity,"evidence.value").is_object(){"partial"}else{"unknown"},"source":"separately_sourced_API_and_client_observations","remainingCharges":if current{remaining}else{None},"usedCharges":null,"usesToday":null,"completedToday":null,"nextReset":null,"apiUsage":api,"clientUsage":{"status":if row.is_null(){"unavailable"}else if current{"observed"}else{"stale"},"currentStateVerified":current,"observedAt":row["observedAt"],"lastObservedAvailableCharges":remaining,"maximumChargesAtObservation":count(&values["maximumCharges"]),"paidResetsUsedToday":if current{count(&values["paidResetsUsedToday"])}else{None},"paidResetsRemainingToday":if current{count(&values["paidResetsRemainingToday"])}else{None},"maximumPaidResetsTodayAtObservation":count(&values["maximumPaidResetsToday"]),"nextChargeSecondsAtObservation":recharge,"paidResetSecondsAtObservation":reset,"rechargeAndPaidResetAreSeparate":true,"timerIsLiveCountdown":false},"chargeTracking":{"values":ledger["values"],"observedAt":ledger["observedAt"],"completeDailyUseCount":false,"counterMeaning":"Persistent observed net charge decreases; not proof of completed or claimed Superpairs, paid resets, or all uses today. Replenishment can hide spending; restart/disconnect gaps do not establish changes.","persistentHistory":false},"reasonCode":if current&&remaining.is_some(){"explicit_current_experiment_charge_counter"}else{"requires_recent_experiment_menu_for_current_charges"}}),
    )
}
pub fn valid_table(v: &Value) -> bool {
    !obj(v).is_empty()
        && !(v.get("maximumCharges").is_some()
            && v.get("availableCharges").is_some()
            && count(&v["availableCharges"]) > count(&v["maximumCharges"]))
}
pub fn valid_tracking(v: &Value, t: i64) -> bool {
    v["ledgerCreatedAt"].integer().is_some_and(|n| n <= t)
        && has(v, "lastObservedCharges") == has(v, "lastChargesObservedAt")
        && (!has(v, "lastChargesObservedAt")
            || v["lastChargesObservedAt"]
                .integer()
                .is_some_and(|n| n <= t && n >= v["ledgerCreatedAt"].integer().unwrap_or(0)))
}
