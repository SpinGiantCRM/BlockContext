use crate::{diagnostics::Diagnostics, factual::project_own, inventory::digest, types::*};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
pub const DEMO_UUID: &str = "0123456789abcdef0123456789abcdef";
#[derive(Default)]
struct Budget {
    calls: Vec<i64>,
    blocked_until: i64,
    remaining: Option<u64>,
    reset_at: i64,
}
pub struct Upstream {
    pub source: &'static str,
    key: String,
    expires: Option<i64>,
    limit: usize,
    http: reqwest::Client,
    budget: Mutex<Budget>,
    pub diagnostics: Arc<Diagnostics>,
}
impl Upstream {
    pub fn new(
        mode: &str,
        key: String,
        expires: Option<&str>,
        limit: usize,
        diagnostics: Arc<Diagnostics>,
    ) -> Result<Arc<Self>> {
        if !["live", "demo"].contains(&mode) || !(1..=10000).contains(&limit) {
            return Err(AppError::new(
                "INVALID_CONFIG",
                "Use live/demo mode and an integer request budget from 1 to 10000.",
            ));
        }
        let expiry = expires
            .map(|s| {
                parse_time(s).ok_or_else(|| {
                    AppError::new(
                        "INVALID_CONFIG",
                        "The key expiry must be an ISO date with an explicit timezone.",
                    )
                })
            })
            .transpose()?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("BlockContext/0.11.0")
            .build()
            .map_err(|_| {
                AppError::new("INVALID_CONFIG", "Could not initialize the HTTPS client.")
            })?;
        Ok(Arc::new(Self {
            source: if mode == "demo" {
                "synthetic_demo"
            } else {
                "hypixel"
            },
            key,
            expires: expiry,
            limit,
            http,
            budget: Mutex::new(Budget::default()),
            diagnostics,
        }))
    }
    pub async fn get(&self, url: &str, authenticated: bool, endpoint: &str) -> Result<Value> {
        let start = Instant::now();
        let mut status = None;
        let mut guard = if authenticated {
            Some(self.budget.lock().await)
        } else {
            None
        };
        let result=async{
 if let Some(budget)=guard.as_mut(){let t=now();if self.key.is_empty(){return Err(AppError::new("HYPIXEL_KEY_REQUIRED","Set HYPIXEL_API_KEY privately in the local .env file to enable API snapshots."))}
if self.expires.is_some_and(|expiry|t>=expiry){return Err(AppError::new("HYPIXEL_KEY_EXPIRED","The configured Hypixel development key has expired. Replace it privately in the local .env file."))}budget.calls.retain(|n|t-*n<300000);if t>=budget.reset_at{budget.remaining=None}
if budget.blocked_until>t||budget.calls.len()>=self.limit||budget.remaining.is_some_and(|n|n<=2){let retry=budget.blocked_until.max(if budget.calls.len()>=self.limit{budget.calls[0]+300000}else{t}).max(if budget.remaining.is_some_and(|n|n<=2){budget.reset_at}else{t});return Err(AppError::retry("RATE_LIMITED","Hypixel request budget is reserved or temporarily exhausted. Try again later.",((retry-t+999)/1000).max(1)as u64))}budget.calls.push(t);}
 let mut request=self.http.get(url);if authenticated{request=request.header("API-Key",&self.key)}
let mut response=request.send().await.map_err(|e|if e.is_timeout(){AppError::retry("UPSTREAM_TIMEOUT","The upstream request exceeded its time limit.",15)}else{AppError::retry("UPSTREAM_UNAVAILABLE","The upstream service could not be reached.",15)})?;let code=response.status().as_u16();status=Some(code);if let Some(budget)=guard.as_mut(){let header=|name:&str|response.headers().get(name).and_then(|v|v.to_str().ok()).filter(|s|re("^[0-9]+$",s)).and_then(|s|s.parse::<u64>().ok());if let Some(rem)=header("ratelimit-remaining"){budget.remaining=Some(rem)}budget.reset_at=header("ratelimit-reset").map(|n|now()+n.min(3600)as i64*1000).unwrap_or_else(||if budget.remaining.is_some(){now()+300000}else{budget.reset_at});}
 if code==429{let retry=response.headers().get("retry-after").and_then(|v|v.to_str().ok()).filter(|s|re("^[0-9]+$",s)).and_then(|s|s.parse::<u64>().ok()).unwrap_or(60).min(3600);if let Some(budget)=guard.as_mut(){budget.blocked_until=now()+retry as i64*1000;}return Err(AppError::retry("RATE_LIMITED","The upstream service is throttling requests.",retry))}
if code==401||code==403{return Err(AppError::new("HYPIXEL_AUTH_FAILED","The upstream service rejected access. Check the application and key."))}
if code==404||code==204{return Err(AppError::new("PLAYER_NOT_FOUND","No matching Minecraft player was found."))}
if !response.status().is_success(){let mut e=AppError::new("UPSTREAM_UNAVAILABLE","The upstream service returned an unsuccessful response.");e.transient=code>=500;return Err(e)}
if response.content_length().is_some_and(|n|n>12*1024*1024){return Err(AppError::new("UPSTREAM_TOO_LARGE","The upstream response exceeded the size limit."))}
let mut blob=Vec::new();while let Some(chunk)=response.chunk().await.map_err(|e|if e.is_timeout(){AppError::retry("UPSTREAM_TIMEOUT","The upstream request exceeded its time limit.",15)}else{AppError::retry("UPSTREAM_UNAVAILABLE","The upstream response could not be read.",15)})?{if blob.len()+chunk.len()>12*1024*1024{return Err(AppError::new("UPSTREAM_TOO_LARGE","The upstream response exceeded the size limit."))}blob.extend_from_slice(&chunk);}
let data:Value=serde_json::from_slice(&blob).map_err(|_|AppError::retry("UPSTREAM_INVALID","The upstream response was not valid JSON.",15))?;if !data.is_object(){return Err(AppError::retry("UPSTREAM_INVALID","The upstream response was not a JSON object.",15))}
if data["success"]==false{return Err(AppError::new("UPSTREAM_REJECTED","Hypixel rejected the request."))}Ok(data)}.await;
        self.diagnostics.upstream(
            endpoint,
            start.elapsed().as_secs_f64() * 1000.,
            status,
            result.as_ref().err(),
            guard.as_ref().and_then(|b| b.remaining),
        );
        result
    }
    pub async fn resolve(&self, player: &str) -> Result<Value> {
        validate_player(player)?;
        if self.source == "synthetic_demo" {
            if player.eq_ignore_ascii_case("DemoPlayer")
                || uuid(player).as_deref() == Some(DEMO_UUID)
            {
                return Ok(json!({"uuid":DEMO_UUID,"name":"DemoPlayer"}));
            }
            return Err(AppError::new(
                "DEMO_PLAYER_ONLY",
                "Demo mode only includes a synthetic DemoPlayer profile.",
            ));
        }
        if let Some(uuid) = uuid(player) {
            return Ok(json!({"uuid":uuid,"name":null}));
        }
        let data = self
            .get(
                &format!("https://api.mojang.com/users/profiles/minecraft/{player}"),
                false,
                "identity",
            )
            .await?;
        let id = data["id"].as_str().and_then(uuid).ok_or_else(|| {
            AppError::retry(
                "UPSTREAM_INVALID",
                "Minecraft returned an invalid player identity.",
                15,
            )
        })?;
        Ok(json!({"uuid":id,"name":text(&data["name"],16)}))
    }
    pub async fn profiles(&self, id: &str) -> Result<Value> {
        if self.source == "synthetic_demo" {
            if id != DEMO_UUID {
                return Err(AppError::new(
                    "DEMO_PLAYER_ONLY",
                    "No fixture exists for that UUID.",
                ));
            }
            let fixture: Value =
                serde_json::from_str(include_str!("../../fixtures/synthetic-profile.json"))
                    .unwrap();
            return Ok(fixture["profiles"].clone());
        }
        if uuid(id).as_deref() != Some(id) {
            return Err(AppError::new("INVALID_PLAYER", "Provide a valid UUID."));
        }
        let data = self
            .get(
                &format!("https://api.hypixel.net/v2/skyblock/profiles?uuid={id}"),
                true,
                "profiles",
            )
            .await?;
        if data["profiles"].is_null() {
            return Ok(json!([]));
        }
        let profiles = data["profiles"]
            .as_array()
            .filter(|a| a.len() <= 8)
            .ok_or_else(|| {
                AppError::retry(
                    "UPSTREAM_INVALID",
                    "Hypixel returned an invalid profile list.",
                    15,
                )
            })?;
        Ok(json!(
            profiles
                .iter()
                .map(|p| project_profile(p, id))
                .collect::<Result<Vec<_>>>()?
        ))
    }
    pub async fn skills(&self) -> Result<Value> {
        if self.source == "synthetic_demo" {
            return Ok(
                json!({"skills":Value::Object(["COMBAT","MINING","FARMING","HUNTING"].iter().map(|s|(s.to_string(),json!({"levels":[{"level":1,"totalExpRequired":50},{"level":2,"totalExpRequired":175},{"level":3,"totalExpRequired":375}]}))).collect())}),
            );
        }
        self.get(
            "https://api.hypixel.net/v2/resources/skyblock/skills",
            false,
            "skills",
        )
        .await
    }
    pub async fn items(&self) -> Result<Value> {
        if self.source == "synthetic_demo" {
            return Ok(json!({"items":{}}));
        }
        let raw = self
            .get(
                "https://api.hypixel.net/v2/resources/skyblock/items",
                false,
                "items",
            )
            .await?;
        let items = raw["items"]
            .as_array()
            .filter(|a| a.len() <= 20000)
            .ok_or_else(|| {
                AppError::retry(
                    "UPSTREAM_INVALID",
                    "Hypixel returned an invalid item catalog.",
                    15,
                )
            })?;
        let mut out = serde_json::Map::new();
        for item in items {
            let Some(id) = text(&item["id"], 96).filter(|id| re("^[A-Z0-9_:.-]{1,96}$", id)) else {
                continue;
            };
            out.insert(id,merge(project_own(item,0)?,json!({"name":text(&item["name"],96),"category":text(&item["category"],96),"tier":text(&item["tier"],96),"npcSellPrice":n(&item["npc_sell_price"])})));
        }
        Ok(json!({"lastUpdated":n(&raw["lastUpdated"]),"items":out}))
    }
    pub async fn player_metadata(&self, id: &str) -> Result<Value> {
        if self.source == "synthetic_demo" {
            return Ok(
                json!({"newPackageRank":"NONE","rank":"NORMAL","packageRank":"NONE","monthlyPackageRank":"NONE"}),
            );
        }
        let raw = self
            .get(
                &format!("https://api.hypixel.net/v2/player?uuid={id}"),
                true,
                "rank",
            )
            .await?;
        if raw["player"].is_null() {
            return Ok(json!({}));
        }
        let p = &raw["player"];
        if obj(p).is_empty() {
            return Err(AppError::retry(
                "UPSTREAM_INVALID",
                "Hypixel returned invalid player metadata.",
                15,
            ));
        }
        Ok(
            json!({"rank":text(&p["rank"],96),"packageRank":text(&p["packageRank"],96),"newPackageRank":text(&p["newPackageRank"],96),"monthlyPackageRank":text(&p["monthlyPackageRank"],96),"skyblock":project_own(at(p,"stats.SkyBlock"),0)?,"achievements":project_own(&Value::Object(obj(&p["achievements"]).into_iter().filter(|(k,_)|k.starts_with("skyblock_")).collect()),0)?,"achievementsOneTime":p["achievementsOneTime"].as_array().map(|a|a.iter().filter(|v|v.as_str().is_some_and(|s|s.starts_with("skyblock_"))).cloned().collect::<Vec<_>>())}),
        )
    }
    pub async fn garden(&self, id: &str) -> Result<Value> {
        if self.source == "synthetic_demo" {
            return Ok(
                json!({"garden_experience":10,"resources_collected":{"WHEAT":73},"crop_upgrade_levels":{},"garden_upgrades":{},"unlocked_plots_ids":["beginner_2"],"commission_data":{"visits":{},"completed":{}}}),
            );
        }
        let raw = match self
            .get(
                &format!("https://api.hypixel.net/v2/skyblock/garden?profile={id}"),
                true,
                "garden",
            )
            .await
        {
            Err(e) if e.code == "PLAYER_NOT_FOUND" => return Ok(Value::Null),
            v => v?,
        };
        let g = &raw["garden"];
        if g.is_null() {
            return Ok(Value::Null);
        }
        if obj(g).is_empty() || g["uuid"].as_str().and_then(uuid).as_deref() != Some(id) {
            return Err(AppError::retry(
                "UPSTREAM_INVALID",
                "Hypixel returned an invalid Garden identity.",
                15,
            ));
        }
        project_own(
            &Value::Object(obj(g).into_iter().filter(|(k, _)| k != "uuid").collect()),
            0,
        )
    }
    pub async fn reference(&self, kind: &str) -> Result<Value> {
        if self.source == "synthetic_demo" {
            return Err(AppError::new(
                "RESOURCE_UNAVAILABLE",
                "This source has no reference endpoint.",
            ));
        }
        let path = match kind {
            "collections" => "/v2/resources/skyblock/collections",
            "skills" => "/v2/resources/skyblock/skills",
            "items" => "/v2/resources/skyblock/items",
            "election" => "/v2/resources/skyblock/election",
            "bingo" => "/v2/resources/skyblock/bingo",
            "news" => "/v2/skyblock/news",
            "fire_sales" => "/v2/skyblock/firesales",
            "ended_auctions" => "/v2/skyblock/auctions_ended",
            _ => {
                return Err(AppError::new(
                    "INVALID_RESOURCE",
                    "Choose a supported SkyBlock reference.",
                ));
            }
        };
        project_own(
            &self
                .get(&format!("https://api.hypixel.net{path}"), false, kind)
                .await?,
            0,
        )
    }
    pub async fn supplement(&self, kind: &str, owner: &str, profile: &str) -> Result<Value> {
        if uuid(owner).as_deref() != Some(owner) || uuid(profile).as_deref() != Some(profile) {
            return Err(AppError::new(
                "INVALID_PROFILE",
                "Provide valid player and profile identities.",
            ));
        }
        if self.source == "synthetic_demo" {
            return Err(AppError::new(
                "SUPPLEMENT_UNAVAILABLE",
                "This source does not expose supplemental endpoints.",
            ));
        }
        match kind {
            "museum" => {
                let raw = self
                    .get(
                        &format!("https://api.hypixel.net/v2/skyblock/museum?profile={profile}"),
                        true,
                        "museum",
                    )
                    .await?;
                Ok(json!({"ownMember":project_own(&raw["members"][owner],0)?,"endpoint":"museum"}))
            }
            "bingo" => project_own(
                &self
                    .get(
                        &format!("https://api.hypixel.net/v2/skyblock/bingo?uuid={owner}"),
                        true,
                        "bingo",
                    )
                    .await?,
                0,
            ),
            "auctions" => {
                let raw = self
                    .get(
                        &format!("https://api.hypixel.net/v2/skyblock/auction?player={owner}"),
                        true,
                        "own_auctions",
                    )
                    .await?;
                let auctions = raw["auctions"].as_array().map(|a| {
                    json!(
                        a.iter()
                            .filter(|a| a["profile_id"] == profile)
                            .collect::<Vec<_>>()
                    )
                });
                Ok(
                    json!({"auctions":project_own(&auctions.unwrap_or(Value::Null),0)?,"profileScoped":true}),
                )
            }
            _ => Err(AppError::new(
                "SUPPLEMENT_UNAVAILABLE",
                "Unsupported supplemental endpoint.",
            )),
        }
    }
    pub async fn bazaar(&self) -> Result<Value> {
        if self.source == "synthetic_demo" {
            return Ok(
                json!({"lastUpdated":now(),"products":{"WHEAT":{"buy_summary":[{"amount":10,"pricePerUnit":8,"orders":1}],"sell_summary":[{"amount":10,"pricePerUnit":3,"orders":1}],"quick_status":{"buyPrice":8,"sellPrice":3,"buyVolume":10,"sellVolume":10}}}}),
            );
        }
        let raw = self
            .get(
                "https://api.hypixel.net/v2/skyblock/bazaar",
                false,
                "bazaar",
            )
            .await?;
        if !raw["products"]
            .as_object()
            .is_some_and(|o| o.len() <= 10000)
        {
            return Err(AppError::retry(
                "UPSTREAM_INVALID",
                "Hypixel returned an invalid Bazaar snapshot.",
                15,
            ));
        }
        Ok(json!({"lastUpdated":n(&raw["lastUpdated"]),"products":raw["products"]}))
    }
    pub async fn auction_page(&self, page: usize) -> Result<Value> {
        if page >= 128 {
            return Err(AppError::new(
                "INVALID_AUCTION_PAGE",
                "Invalid auction page.",
            ));
        }
        if self.source == "synthetic_demo" {
            return Ok(
                json!({"page":0,"totalPages":1,"totalAuctions":0,"lastUpdated":now(),"auctions":[]}),
            );
        }
        let raw = self
            .get(
                &format!("https://api.hypixel.net/v2/skyblock/auctions?page={page}"),
                false,
                "auctions",
            )
            .await?;
        if raw["page"].unsigned() != Some(page as u64)
            || !raw["auctions"].as_array().is_some_and(|a| a.len() <= 1000)
        {
            return Err(AppError::retry(
                "UPSTREAM_INVALID",
                "Invalid auction page payload.",
                15,
            ));
        }
        Ok(
            json!({"page":raw["page"],"totalPages":raw["totalPages"],"totalAuctions":raw["totalAuctions"],"lastUpdated":raw["lastUpdated"],"auctions":arr(&raw["auctions"]).iter().map(|a|json!({"uuid":a["uuid"],"bin":a["bin"],"claimed":a["claimed"],"end":a["end"],"starting_bid":a["starting_bid"],"tier":a["tier"],"item_bytes":a["item_bytes"]})).collect::<Vec<_>>()}),
        )
    }
}
pub fn project_profile(p: &Value, id: &str) -> Result<Value> {
    fn tag(v: &Value) -> Value {
        let mut row = Value::Object(obj(v));
        for (src, dest) in [
            ("run_id", "run_instance_id"),
            ("chest_id", "chest_instance_id"),
        ] {
            if let Some(s) = v[src].as_str() {
                row[dest] = json!(digest(s))
            }
        }
        row
    }
    let mut own = Value::Object(obj(&p["members"][id]));
    if own["dungeons"].is_object() {
        let d = &mut own["dungeons"];
        d["treasures"] = Value::Object(obj(&d["treasures"]));
        let treasure = &mut d["treasures"];
        for key in ["runs", "chests"] {
            if treasure.get(key).is_none() {
                treasure[key] = json!({});
            }
        }
        if let Some(runs) = treasure["runs"].as_array() {
            treasure["runs"] = json!(
                runs.iter()
                    .map(|v| {
                        let mut row = tag(v);
                        let party = arr(&row["participants"]);
                        row["reportedPartySize"] =
                            json!(row["participants"].as_array().map(Vec::len));
                        row["ownParticipant"] = party
                            .iter()
                            .find(|p| p["player_uuid"] == id)
                            .cloned()
                            .unwrap_or(Value::Null);
                        row
                    })
                    .collect::<Vec<_>>()
            );
        }
        if let Some(chests) = treasure["chests"].as_array() {
            treasure["chests"] = json!(chests.iter().map(tag).collect::<Vec<_>>());
        }
        d["dungeon_types"] = Value::Object(obj(&d["dungeon_types"]));
        if let Some(types) = d["dungeon_types"].as_object() {
            d["dungeon_types"]=Value::Object(types.iter().map(|(kind,v)|{let runs=Value::Object(obj(&v["best_runs"]).into_iter().map(|(floor,rs)|{let rs=rs.as_array().map(|a|json!(a.iter().map(|r|merge(r.clone(),json!({"reportedPartySize":r["teammates"].as_array().map(Vec::len)}))).collect::<Vec<_>>())).unwrap_or(rs);(floor,rs)}).collect());(kind.clone(),merge(v.clone(),json!({"best_runs":runs})))}).collect());
        }
    }
    let own = project_own(&own, 0)?;
    let mut shared = project_own(
        &Value::Object(
            obj(p)
                .into_iter()
                .filter(|(k, _)| {
                    ![
                        "members",
                        "profile_id",
                        "cute_name",
                        "selected",
                        "game_mode",
                    ]
                    .contains(&k.as_str())
                })
                .collect(),
        ),
        0,
    )?;
    for key in ["profile_id", "cute_name", "selected", "game_mode"] {
        if let Some(v) = p.get(key) {
            shared[key] = v.clone()
        }
    }
    shared["members"] = json!({id:own});
    Ok(shared)
}
