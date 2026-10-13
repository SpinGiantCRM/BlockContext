use crate::types::*;
use futures::{
    FutureExt,
    future::{BoxFuture, Shared},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub struct Cached {
    pub value: Value,
    pub fetched_at: i64,
    pub cache: &'static str,
    pub warning: Option<String>,
}
type Flight = Shared<BoxFuture<'static, Result<Cached>>>;
pub struct Cache {
    db: Mutex<Connection>,
    flights: Mutex<HashMap<String, Flight>>,
    failures: Mutex<HashMap<String, (i64, AppError)>>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}
fn db_error(_: rusqlite::Error) -> AppError {
    AppError::new(
        "CACHE_UNAVAILABLE",
        "The private snapshot store is unavailable.",
    )
}
impl Cache {
    pub fn open(path: &Path) -> Result<Arc<Self>> {
        Self::with_clock(path, Arc::new(now))
    }
    pub fn with_clock(path: &Path, clock: Arc<dyn Fn() -> i64 + Send + Sync>) -> Result<Arc<Self>> {
        if path != Path::new(":memory:")
            && let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
        {
            crate::client::private_dir(parent).map_err(|_| {
                AppError::new(
                    "CACHE_UNAVAILABLE",
                    "Cannot create private cache directory.",
                )
            })?;
        }
        if path != Path::new(":memory:")
            && std::fs::symlink_metadata(path).is_ok_and(|m| !m.is_file())
        {
            return Err(AppError::new(
                "CACHE_UNAVAILABLE",
                "Cache must be a regular private file.",
            ));
        }
        let db = Connection::open(path).map_err(db_error)?;
        db.execute_batch("PRAGMA journal_mode=DELETE; CREATE TABLE IF NOT EXISTS snapshot_checks(key TEXT PRIMARY KEY,checked_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS cache(key TEXT PRIMARY KEY,value TEXT NOT NULL,fetched_at INTEGER NOT NULL,expires_at INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS deferred_poll_checks(owner TEXT NOT NULL,profile TEXT NOT NULL,id TEXT NOT NULL,target TEXT NOT NULL,cadence TEXT NOT NULL,created_at INTEGER NOT NULL,next_due INTEGER NOT NULL,last_evaluated INTEGER,last_due INTEGER,PRIMARY KEY(owner,profile,id));").map_err(db_error)?;
        #[cfg(unix)]
        if path != Path::new(":memory:") {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(
                |_| AppError::new("CACHE_UNAVAILABLE", "Cannot protect private cache file."),
            )?;
        }
        Ok(Arc::new(Self {
            db: Mutex::new(db),
            flights: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
            clock,
        }))
    }
    pub fn time(&self) -> i64 {
        (self.clock)()
    }
    pub fn peek(&self, key: &str, max_age: i64) -> Option<Cached> {
        let db = self.db.lock().ok()?;
        let row: Option<(String, i64)> = db
            .query_row(
                "SELECT value,fetched_at FROM cache WHERE key=?",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .ok()?;
        let (s, t) = row?;
        if self.time() < t || self.time() - t >= max_age {
            return None;
        }
        Some(Cached {
            value: serde_json::from_str(&s).ok()?,
            fetched_at: t,
            cache: "stale",
            warning: None,
        })
    }
    pub async fn get<F>(
        self: &Arc<Self>,
        key: String,
        fresh: i64,
        max_age: i64,
        load: F,
        use_snapshot_time: bool,
    ) -> Result<Cached>
    where
        F: std::future::Future<Output = Result<Value>> + Send + 'static,
    {
        let time = self.time();
        {
            let db = self.db.lock().unwrap();
            db.execute("DELETE FROM cache WHERE expires_at<=?", [time])
                .map_err(db_error)?;
        }
        let existing = self.peek(&key, max_age);
        if let Some(c) = &existing
            && time - c.fetched_at < fresh
        {
            let mut c = c.clone();
            c.cache = "fresh";
            return Ok(c);
        }
        if let Some((until, e)) = self.failures.lock().unwrap().get(&key).cloned()
            && until > time
        {
            return self.fallback(existing, e, max_age);
        }
        let flight = {
            let mut flights = self.flights.lock().unwrap();
            if let Some(f) = flights.get(&key) {
                f.clone()
            } else {
                let this = self.clone();
                let k = key.clone();
                let future=async move {
    let result=async {let value=load.await?;let fetched_at=if use_snapshot_time{let retrieved=value.get("retrievedAt").and_then(Value::as_i64).ok_or_else(||AppError::new("SNAPSHOT_EXPIRED","Missing snapshot time."))?;value.get("updatedAt").and_then(Value::as_i64).map(|t|t.min(retrieved)).unwrap_or(retrieved)}else{this.time()};
     if fetched_at>this.time()||this.time()-fetched_at>=max_age{return Err(AppError::retry("SNAPSHOT_EXPIRED","The snapshot expired while being prepared.",15))}
     let s=serde_json::to_string(&value).map_err(|_|AppError::new("CACHE_INVALID","Invalid snapshot."))?;if s.len()>8*1024*1024{return Err(AppError::new("RESPONSE_TOO_LARGE","The normalized snapshot exceeded its storage limit."))}
     {let db=this.db.lock().unwrap();db.execute("INSERT INTO cache(key,value,fetched_at,expires_at) VALUES(?,?,?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value,fetched_at=excluded.fetched_at,expires_at=excluded.expires_at",params![k,s,fetched_at,fetched_at+max_age]).map_err(db_error)?;db.execute("DELETE FROM cache WHERE key IN (SELECT key FROM cache ORDER BY fetched_at DESC,key LIMIT -1 OFFSET 512)",[]).map_err(db_error)?;}
     this.failures.lock().unwrap().remove(&k);Ok(Cached{value,fetched_at,cache:"miss",warning:None})
    }.await;
    let result=match result{Ok(v)=>Ok(v),Err(e)=>{let mut failures=this.failures.lock().unwrap();failures.insert(k.clone(),(this.time()+(e.retry_after_seconds.unwrap_or(0)*1000).clamp(15000,300000) as i64,e.clone()));if failures.len()>512&& let Some(key)=failures.keys().next().cloned(){failures.remove(&key);}drop(failures);this.fallback(existing,e,max_age)}};
    this.flights.lock().unwrap().remove(&k);result
   }.boxed().shared();
                flights.insert(key.clone(), future.clone());
                future
            }
        };
        flight.await
    }
    fn fallback(&self, existing: Option<Cached>, e: AppError, max_age: i64) -> Result<Cached> {
        if let Some(mut c) = existing
            && e.transient
            && self.time() >= c.fetched_at
            && self.time() - c.fetched_at < max_age
        {
            c.cache = "stale";
            c.warning = Some(format!(
                "{}: refresh failed; using an explicitly stale snapshot.",
                e.code
            ));
            return Ok(c);
        }
        Err(e)
    }
    pub async fn snapshot<F>(
        self: &Arc<Self>,
        key: String,
        fresh: i64,
        max_age: i64,
        load: F,
        mode: &str,
    ) -> Result<Cached>
    where
        F: std::future::Future<Output = Result<Value>> + Send + 'static,
    {
        if mode == "cached" {
            if let Some(mut c) = self.peek(&key, max_age) {
                c.cache = if self.time() - c.fetched_at < fresh {
                    "fresh"
                } else {
                    "stale"
                };
                if c.cache == "stale" {
                    c.warning = Some("Cached player snapshot; no API refresh was attempted.".into())
                }
                return Ok(c);
            }
            let checked = self
                .db
                .lock()
                .unwrap()
                .query_row("SELECT key FROM snapshot_checks WHERE key=?", [&key], |r| {
                    r.get::<_, String>(0)
                })
                .optional()
                .map_err(db_error)?
                .is_some();
            if checked {
                return Err(AppError::new(
                    "SNAPSHOT_REFRESH_REQUIRED",
                    "The cached player snapshot has expired. Request snapshot mode for an explicit on-demand check; use collector live reads for polling.",
                ));
            }
        }
        let c = self.get(key.clone(), fresh, max_age, load, true).await?;
        let db = self.db.lock().unwrap();
        db.execute("INSERT INTO snapshot_checks(key,checked_at) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET checked_at=excluded.checked_at",params![key,self.time()]).map_err(db_error)?;
        db.execute("DELETE FROM snapshot_checks WHERE key IN (SELECT key FROM snapshot_checks ORDER BY checked_at DESC LIMIT -1 OFFSET 2048)",[]).map_err(db_error)?;
        Ok(c)
    }
    fn valid_schedule(owner: &str, profile: &str, id: Option<&str>) -> Result<()> {
        if !re("^[a-z_]+:[a-f0-9]{32}$", owner)
            || !re("^[a-z][a-z ]{0,31}$", profile)
            || id.is_some_and(|v| !re("^[a-z0-9_-]{1,48}$", v))
        {
            return Err(AppError::new(
                "INVALID_SCHEDULE",
                "Use an exact profile name and a short check ID.",
            ));
        }
        Ok(())
    }
    pub fn schedule_set(
        &self,
        owner: &str,
        profile: &str,
        id: &str,
        target: &str,
        cadence: &Value,
    ) -> Result<Value> {
        Self::valid_schedule(owner, profile, Some(id))?;
        if !CHECK_TARGETS.contains(&target)
            || (cadence["kind"] == "hotm_reset_reference" && target != "hotm_bonus_commissions")
        {
            return Err(AppError::new(
                "INVALID_SCHEDULE",
                "Choose a supported profile-only check.",
            ));
        }
        let t = self.time();
        let next = next_check(cadence, t)?;
        {
            let db = self.db.lock().unwrap();
            let exists = db
                .query_row(
                    "SELECT id FROM deferred_poll_checks WHERE owner=? AND profile=? AND id=?",
                    params![owner, profile, id],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(db_error)?
                .is_some();
            if !exists {
                let local: i64 = db
                    .query_row(
                        "SELECT count(*) FROM deferred_poll_checks WHERE owner=? AND profile=?",
                        params![owner, profile],
                        |r| r.get(0),
                    )
                    .map_err(db_error)?;
                let total: i64 = db
                    .query_row("SELECT count(*) FROM deferred_poll_checks", [], |r| {
                        r.get(0)
                    })
                    .map_err(db_error)?;
                if local >= 16 || total >= 256 {
                    return Err(AppError::new(
                        "SCHEDULE_LIMIT",
                        "Remove an existing saved check before adding another.",
                    ));
                }
            }
            db.execute("INSERT INTO deferred_poll_checks(owner,profile,id,target,cadence,created_at,next_due) VALUES(?,?,?,?,?,?,?) ON CONFLICT(owner,profile,id) DO UPDATE SET target=excluded.target,cadence=excluded.cadence,created_at=excluded.created_at,next_due=excluded.next_due,last_evaluated=NULL,last_due=NULL",params![owner,profile,id,target,cadence.to_string(),t,next]).map_err(db_error)?;
        }
        Ok(self
            .schedule_list(owner, profile)?
            .into_iter()
            .find(|v| v["id"] == id)
            .unwrap())
    }
    pub fn schedule_list(&self, owner: &str, profile: &str) -> Result<Vec<Value>> {
        Self::valid_schedule(owner, profile, None)?;
        Ok(self.schedule_rows(owner,Some(profile),false)?.into_iter().map(|r|json!({"id":r.id,"target":r.target,"cadence":r.cadence,"nextDueAt":if r.next_due<0{None}else{Some(iso(r.next_due))},"due":r.next_due>=0&&r.next_due<=self.time()&&r.created_at<=self.time(),"lastEvaluatedAt":r.last_evaluated.map(iso),"lastDueAt":r.last_due.map(iso),"trigger":"next_explicit_profile_snapshot","backgroundPolling":false})).collect())
    }
    pub fn schedule_remove(&self, owner: &str, profile: &str, id: &str) -> Result<bool> {
        Self::valid_schedule(owner, profile, Some(id))?;
        Ok(self
            .db
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM deferred_poll_checks WHERE owner=? AND profile=? AND id=?",
                params![owner, profile, id],
            )
            .map_err(db_error)?
            > 0)
    }
    pub fn schedule_due(&self, owner: &str, profile: Option<&str>) -> Result<Vec<Check>> {
        Self::valid_schedule(owner, profile.unwrap_or("any"), None)?;
        self.schedule_rows(owner, profile, true)
    }
    fn schedule_rows(&self, owner: &str, profile: Option<&str>, due: bool) -> Result<Vec<Check>> {
        let db = self.db.lock().unwrap();
        let mut statement=db.prepare("SELECT owner,profile,id,target,cadence,created_at,next_due,last_evaluated,last_due FROM deferred_poll_checks WHERE owner=? ORDER BY next_due,id").map_err(db_error)?;
        let rows = statement
            .query_map([owner], |r| {
                Ok(Check {
                    owner: r.get(0)?,
                    profile: r.get(1)?,
                    id: r.get(2)?,
                    target: r.get(3)?,
                    cadence: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or(Value::Null),
                    created_at: r.get(5)?,
                    next_due: r.get(6)?,
                    last_evaluated: r.get(7)?,
                    last_due: r.get(8)?,
                })
            })
            .map_err(db_error)?;
        rows.filter_map(|r| match r {
            Ok(v)
                if profile.is_none_or(|p| p == v.profile)
                    && (!due
                        || (v.next_due >= 0
                            && v.next_due <= self.time()
                            && v.created_at <= self.time())) =>
            {
                Some(Ok(v))
            }
            Ok(_) => None,
            Err(e) => Some(Err(db_error(e))),
        })
        .collect()
    }
    pub fn schedule_complete(&self, r: &Check, fetched_at: i64) -> Result<bool> {
        if fetched_at < r.next_due || fetched_at > self.time() {
            return Ok(false);
        }
        let next = if r.cadence["kind"] == "once" {
            -1
        } else {
            next_check(&r.cadence, self.time())?
        };
        Ok(self.db.lock().unwrap().execute("UPDATE deferred_poll_checks SET next_due=?,last_evaluated=?,last_due=? WHERE owner=? AND profile=? AND id=? AND next_due=? AND cadence=? AND created_at=?",params![next,self.time(),r.next_due,r.owner,r.profile,r.id,r.next_due,r.cadence.to_string(),r.created_at]).map_err(db_error)?>0)
    }
}
pub const CHECK_TARGETS: &[&str] = &[
    "hotm_bonus_commissions",
    "currencies",
    "stats",
    "skills",
    "mining",
    "dailies",
];
#[derive(Debug, Clone)]
pub struct Check {
    pub owner: String,
    pub profile: String,
    pub id: String,
    pub target: String,
    pub cadence: Value,
    pub created_at: i64,
    pub next_due: i64,
    pub last_evaluated: Option<i64>,
    pub last_due: Option<i64>,
}
pub fn next_check(c: &Value, t: i64) -> Result<i64> {
    let invalid = || {
        AppError::new(
            "INVALID_SCHEDULE",
            "Use a bounded ISO timestamp or HH:MM in UTC.",
        )
    };
    match c["kind"].as_str() {
        Some("once") => {
            let n = c["at"].as_str().and_then(parse_time).ok_or_else(invalid)?;
            if n < 0 || n > t + 366 * 86400000 {
                return Err(invalid());
            }
            Ok(n)
        }
        Some("daily_utc") | Some("hotm_reset_reference") => {
            let time = if c["kind"] == "hotm_reset_reference" {
                "05:00"
            } else {
                c["timeUtc"].as_str().ok_or_else(invalid)?
            };
            if !re("^([01][0-9]|2[0-3]):[0-5][0-9]$", time) {
                return Err(invalid());
            }
            let h: i64 = time[..2].parse().map_err(|_| invalid())?;
            let m: i64 = time[3..].parse().map_err(|_| invalid())?;
            let today = t.div_euclid(86400000) * 86400000 + h * 3600000 + m * 60000;
            Ok(if today > t { today } else { today + 86400000 })
        }
        _ => Err(invalid()),
    }
}
