use crate::{factual::REQUIREMENTS, types::*};
use serde_json::{Value, json};
pub fn reference_day(t: i64) -> i64 {
    (t - 18000000).div_euclid(86400000) * 86400000 + 18000000
}
pub fn reset_assessment(client: &Value, t: i64) -> Value {
    let ledger = at(client, "observations.commissionTracking.values");
    let current = at(client, "observations.commissions");
    let values = &current["values"];
    let last = ledger["lastRemainingObservedAt"]
        .integer()
        .or_else(|| current["observedAt"].as_str().and_then(parse_time));
    let expected = reference_day(t);
    let window=ledger["bonusRenewalAfter"].integer().zip(ledger["bonusRenewalObservedAt"].integer()).map(|(after,by)|json!({"after":iso(after),"by":iso(by),"widthMilliseconds":by-after,"referenceBoundaryFallsInside":after<reference_day(by)&&reference_day(by)<=by,"exactServerResetTimeKnown":false}));
    json!({"expectedLastResetAt":iso(expected),"expectedNextResetAt":iso(expected+86400000),"expectedDailyBonuses":4,"reference":{"utcHour":5,"utcMinute":0,"expectedDailyBonuses":4,"serverVerified":false,"source":"https://github.com/NotEnoughUpdates/NotEnoughUpdates/blob/9b1fcfebc646e9fb69f99006327faa3e734e5f51/src/main/java/io/github/moulberry/notenoughupdates/overlays/TimersOverlay.java","dailyBonusSource":"https://hypixel.net/threads/skyblock-patch-0-11-dwarven-mines.3749492/","basis":"community_reset_reference_and_historical_official_daily_bonus_rule"},"resetSinceLastCountObservation":last.map(|n|reference_day(n)!=expected),"bonusReplenishmentObserved":window.is_some(),"lastObservedReplenishment":window,"actualRemainingBonuses":if current["currentStateVerified"]==true{values["remainingBonusCommissions"].clone()}else{Value::Null},"countStatus":if current["currentStateVerified"]==true&&has(values,"remainingBonusCommissions"){"known_current_observation"}else{"unknown_requires_game_counter_or_complete_claim_coverage"},"observedBonusClaimsInReferenceDay":if ledger["referenceDayStart"].integer()==Some(expected){ledger["observedBonusClaimsInReferenceDay"].clone()}else{Value::Null},"referenceDayIsVerifiedServerDay":false,"persistentObservedClaims":ledger["persistentObservedClaims"],"persistentObservedBonusClaims":ledger["persistentObservedBonusClaims"],"persistenceAvailable":ledger["persistenceAvailable"],"completeDailyClaimCount":false,"reasonCode":"expected_reset_does_not_establish_actual_reset_or_four_remaining"})
}
pub fn observed_activities(client: &Value) -> Vec<Value> {
    let mut rows = vec![];
    for (key, row) in obj(&client["observations"]) {
        for fact in arr(at(&row, "values.facts")) {
            for f in arr(&fact["fields"]) {
                let Some(value) =
                    nonnegative(&f["value"]).filter(|n| n.fract() == 0. && *n <= 9007199254740991.)
                else {
                    continue;
                };
                let label = f["label"].as_str().unwrap_or("");
                if !["Bonus Commissions Remaining", "Remaining Charges"].contains(&label) {
                    continue;
                }
                let current = row["currentStateVerified"] == true;
                rows.push(json!({"activityId":format!("{key}.{}",label.to_lowercase().replace(' ',"_")),"name":label,"status":if current{"known"}else{"stale"},"eligible":null,"remainingCharges":value,"completedToday":if current{Some(value==0.)}else{None},"completedAtObservation":value==0.,"observationSource":"client_observation","observedAt":row["observedAt"],"scope":"member","nextReset":null,"rewards":null,"reasonCode":if current{"explicit_current_displayed_remaining_counter"}else{"older_observation_does_not_establish_today"},"evidence":{"sourceKey":key,"label":label}}));
            }
        }
    }
    rows
}
pub fn reconcile(activities: &Value, client: &Value, t: i64) -> Vec<Value> {
    arr(activities).into_iter().map(|activity|{if activity["activityId"]!="hotm_bonus_commissions"{return activity}
let assessment=reset_assessment(client,t);let row=at(client,"observations.commissions");if row.is_null(){return merge(activity,json!({"resetAssessment":assessment}))}
let values=&row["values"];let current=row["currentStateVerified"]==true;let remaining=nonnegative(&values["remainingBonusCommissions"]).filter(|n|n.fract()==0.&&*n<=100.);let bonus=values["dailyBonusAvailable"].as_bool();let seconds=nonnegative(&values["nextResetSecondsAtObservation"]).filter(|n|n.fract()==0.&&*n<=86400.);let reason=if !current{"saved_observation_does_not_establish_today"}else if values["availabilityEvidence"]=="validated_current_reward_previews"&&remaining==Some(0.){"validated_reward_previews_show_bonus_exhausted"}else if remaining.is_some(){if values["remainingCountEvidence"]=="explicit_counter_minus_observed_claims"{"explicit_counter_minus_observed_claims"}else{"explicit_current_displayed_remaining_counter"}}else{"bonus_marker_does_not_establish_remaining_count"};let replenished=has(values,"bonusRenewalObservedAt");let api=activity["evidence"].clone();merge(activity,json!({"resetAssessment":assessment,"status":if current&&(remaining.is_some()||bonus.is_some()){if remaining.is_some(){"known"}else{"partial"}}else{"unknown"},"remainingCharges":if current{remaining}else{None},"completedToday":if current{remaining.map(|n|n==0.).or_else(||bonus.filter(|b|*b).map(|_|false))}else{None},"dailyBonusAvailable":if current{remaining.map(|n|n>0.).or(bonus)}else{None},"usedCharges":null,"totalDailyCharges":null,"nextReset":null,"source":"client_observation","observedAt":row["observedAt"],"currentStateVerified":current,"lastObservation":{"remainingCharges":remaining,"dailyBonusAvailable":remaining.map(|n|n>0.).or(bonus),"nextResetSecondsAtObservation":seconds,"observedAt":row["observedAt"]},"reasonCode":reason,"claimTracking":{"observedClaimTransitions":values["observedClaimTransitions"],"observedBonusClaimTransitions":values["observedBonusClaimTransitions"],"scope":values["claimCountScope"],"completeDailyCount":false,"completionCounterIsNotClaimCounter":true},"bonusReplenishmentEvidence":if replenished{json!({"observedAfter":values["bonusRenewalAfter"],"observedBy":values["bonusRenewalObservedAt"],"exactResetTimeKnown":false,"fullRemainingCountKnown":false})}else{Value::Null},"resetCoverage":{"status":if seconds.is_some(){"observed"}else{"unknown"},"remainingSecondsAtObservation":seconds,"reasonCode":if seconds.is_some(){"displayed_duration_not_a_verified_live_countdown"}else{"reset_not_displayed_or_verified"}},"apiEvidence":api}))}).collect()
}
pub fn data_contract(sections: &Value, client: &Value) -> Value {
    let profile_fields = arr(at(sections, "api_fields.data.fields"));
    let garden_fields = arr(at(sections, "garden.data.apiFields.fields"));
    let mut api = profile_fields.clone();
    api.extend(
        garden_fields
            .iter()
            .map(|f| merge(f.clone(), json!({"domain":"garden"}))),
    );
    let observations = obj(&client["observations"]);
    let mut rows = vec![];
    for (domain, requirements) in REQUIREMENTS.as_object().unwrap() {
        let fields = api
            .iter()
            .filter(|f| f["domain"] == *domain)
            .collect::<Vec<_>>();
        let keys = observations
            .keys()
            .filter(|key| {
                *key == domain
                    || ["equipment", "storage"].contains(&domain.as_str())
                        && ["inventory", "huntaxe", "toolkit"].contains(&key.as_str())
                    || domain == "currencies" && key.starts_with("bank:")
            })
            .cloned()
            .collect::<Vec<_>>();
        let source_coverage = if domain == "garden" {
            Some(
                json!({"profileMember":{"fieldCount":profile_fields.iter().filter(|f|f["domain"]=="garden").count(),"scope":"member"},"gardenEndpoint":{"fieldCount":garden_fields.len(),"scope":"shared_profile","status":sections["garden"]["status"].as_str().unwrap_or("unavailable"),"snapshot":at(sections,"garden.data.snapshot")},"greenhouse":{"scope":"member","status":at(sections,"garden.data.greenhouse.status").as_str().unwrap_or("unavailable"),"snapshot":at(sections,"garden.data.greenhouse.snapshot")}}),
            )
        } else {
            None
        };
        for requirement in arr(requirements) {
            let requirement = requirement.as_str().unwrap();
            let mut api = json!({"fieldCount":fields.len(),"section":sections[domain]["status"].as_str().unwrap_or("unavailable")});
            if let Some(coverage) = &source_coverage {
                api["sourceCoverage"] = coverage.clone()
            }
            rows.push(json!({"id":format!("{domain}.{requirement}"),"domain":domain,"requirement":requirement,"status":if fields.is_empty()&&keys.is_empty(){"unknown"}else{"partial"},"lastKnownValue":null,"unit":null,"scope":"see_source_fields","observedAt":"see_response_metadata.retrievedAt","recommendedFallback":if keys.is_empty(){"View the relevant owner menu if the server sends that field there; otherwise it remains unavailable."}else{"Read the dated collector observation."},"semanticCoverage":"Domain evidence does not certify that this individual requirement is implemented.","api":api,"client":{"observationKeys":keys,"mode":"passive_already_received_owner_data"},"complete":false,"reasonCode":if fields.is_empty()&&keys.is_empty(){"not_exposed_in_available_sources"}else{"inspect_typed_section_and_source_field_coverage"}}));
        }
    }
    json!({"status":"partial","data":{"requirements":rows,"requirementsCount":rows.len(),"domains":REQUIREMENTS.as_object().unwrap().keys().collect::<Vec<_>>(),"complete":false,"excluded":["user_goals","player_preferences","user_reported_facts","advice_history","recommendation_and_method_ranking"],"acquisition":{"api":"Explicit cached snapshots; live mode and rapid repeat snapshot requests are rejected.","client":"Live pollable owner observations; no API fallback, menu automation or server requests.","missing":"Unavailable or unsent data stays unknown, with a source reason."},"apiFieldCoverage":at(sections,"api_fields.data.coverage"),"missingDataReport":obj(sections).into_iter().filter(|(id,s)|id!="data_contract"&&s["status"]!="available").map(|(domain,s)|json!({"domain":domain,"status":s["status"],"reasonCode":at(&s,"data.reasonCode").as_str().unwrap_or("partial_typed_semantics_or_missing_fields"),"fallback":"Inspect source field map and collector observations; never infer zero or empty."})).collect::<Vec<_>>()},"warnings":["This is a requirements coverage registry, not a claim that the whole requested interface is complete. Generic reported fields may be available even when their game semantics or formulas are unverified."]})
}
