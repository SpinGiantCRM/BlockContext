use crate::{
    client::{parse_observation, private_read, validate_snapshot},
    inventory::digest,
    types::*,
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
pub fn project(blob: &[u8], owner: &str, t: i64) -> Result<Vec<u8>> {
    let invalid = || AppError::new("CLIENT_SNAPSHOT_REJECTED", "Snapshot import rejected.");
    if blob.len() > 1024 * 1024 || !re("^[a-f0-9]{32}$", owner) {
        return Err(invalid());
    }
    let raw: Value = serde_json::from_slice(blob).map_err(|_| invalid())?;
    let root = validate_snapshot(&raw).ok_or_else(invalid)?;
    let exported = root["exportedAt"].integer().ok_or_else(invalid)?;
    if exported > t + 5000 || exported < (t - 365 * 86400000).max(0) {
        return Err(invalid());
    }
    let profiles = arr(&root["profiles"]);
    if profiles.is_empty() {
        return Err(invalid());
    }
    let mut result = vec![];
    let mut keys = HashSet::new();
    for p in profiles {
        if p["uuid"] != owner {
            return Err(invalid());
        }
        let profile = p["profileName"].as_str().ok_or_else(invalid)?;
        let key = format!("{owner}:{}", profile.to_lowercase());
        if !keys.insert(key) {
            return Err(invalid());
        }
        let mut observations = json!({});
        for (k, record) in obj(&p["observations"]) {
            let parsed = parse_observation(&k, &record);
            let Some((_, values)) = parsed else {
                if known_key(&k) {
                    return Err(invalid());
                }
                continue;
            };
            let observed = record["observedAt"].integer().ok_or_else(invalid)?;
            if observed > t + 5000 {
                return Err(invalid());
            }
            if observed
                < t - if k == "commissionTracking" {
                    365 * 86400000
                } else {
                    86400000
                }
                || exported < t - 86400000 && k != "commissionTracking"
            {
                continue;
            }
            observations[&k] =
                json!({"observedAt":observed,"readFrom":record["readFrom"],"values":values});
        }
        let mut changes = vec![];
        for c in arr(&p["changes"]) {
            let observed = c["observedAt"].integer().ok_or_else(invalid)?;
            let previous = c["previousObservedAt"].integer().ok_or_else(invalid)?;
            if observed > t + 5000 || previous > t + 5000 {
                return Err(invalid());
            }
            if t - observed < 86400000 {
                changes.push(c)
            }
        }
        result.push(json!({"uuid":owner,"playerName":p["playerName"],"profileName":profile,"observations":observations,"changes":changes}));
    }
    if !root["activeProfile"].is_null()
        && !root["activeProfile"]
            .as_str()
            .is_some_and(|s| keys.contains(s))
    {
        return Err(invalid());
    }
    if exported < t - 86400000
        && !result
            .iter()
            .any(|p| has(p, "observations.commissionTracking"))
    {
        return Err(invalid());
    }
    let mut out = json!({"schemaVersion":1,"collectorVersion":root["collectorVersion"],"exportedAt":exported,"activeProfile":root["activeProfile"],"profiles":result});
    if let Some(a) = root["observedNow"].as_array() {
        if a.iter().any(|v| {
            !v.as_str()
                .is_some_and(|s| re(r"^[A-Za-z0-9_ :().'/#-]{1,96}$", s))
        }) {
            return Err(invalid());
        }
        out["observedNow"] = json!(a);
    }
    for k in ["exportIntervalMs", "evictedObservations"] {
        if let Some(v) = root.get(k) {
            if k == "evictedObservations" && v.unsigned().is_none_or(|n| n > 100000) {
                return Err(invalid());
            }
            out[k] = v.clone();
        }
    }
    let blob = out.to_string().into_bytes();
    if blob.len() > 1024 * 1024 {
        return Err(invalid());
    }
    Ok(blob)
}
fn known_key(k: &str) -> bool {
    [
        "cookie",
        "godPotion",
        "commissions",
        "commissionTracking",
        "commissionMenuEvidence",
        "huntaxe",
        "inventory",
        "toolkit",
        "minion",
        "garden",
        "liveState",
        "sidebar",
        "actionBar",
        "tabStats",
        "mining",
        "foraging",
        "hunting",
        "dailies",
        "stats",
        "pets",
        "collections",
        "accessories",
        "quests",
        "museum",
        "orders",
        "travel",
        "fishing",
        "bestiary",
        "dungeons",
        "slayers",
        "events",
    ]
    .contains(&k)
        || [
            "bank:personalBankCoins",
            "bank:sharedProfileBankCoins",
            "bank:bankCoinsUnclassified",
        ]
        .contains(&k)
        || re(
            r"^storage:(?:(?:Small |Medium |Large |Greater |Jumbo )?Backpack(?: \(Slot #\d{1,2}\))?|Ender Chest(?: \(\d+/\d+\))?|Storage \(\d+/\d+\))$",
            k,
        )
}
pub async fn run(config: &Path) -> anyhow::Result<()> {
    let config: Value = serde_json::from_slice(&private_read(config, 8192)?)
        .map_err(|_| anyhow::anyhow!("Invalid private bridge configuration."))?;
    let host = config["sshHost"]
        .as_str()
        .filter(|s| re("^[A-Za-z0-9_.-]{1,64}@[A-Za-z0-9.-]{1,253}$", s))
        .ok_or_else(|| anyhow::anyhow!("Invalid SSH destination."))?;
    let owner = config["ownerUuid"]
        .as_str()
        .filter(|s| re("^[a-f0-9]{32}$", s))
        .ok_or_else(|| anyhow::anyhow!("Invalid owner identity."))?;
    let identity = PathBuf::from(
        config["sshIdentity"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Private SSH identity required."))?,
    )
    .canonicalize()?;
    let source = PathBuf::from(
        config["source"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Absolute snapshot path required."))?,
    );
    if !source.is_absolute() {
        anyhow::bail!("Absolute snapshot path required.")
    }
    let info = std::fs::metadata(&identity)?;
    if !info.is_file() {
        anyhow::bail!("Private SSH identity required.")
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if info.permissions().mode() & 0o077 != 0 {
            anyhow::bail!("Private SSH identity required.")
        }
    }
    let mut args = vec![
        "-F".to_owned(),
        if cfg!(windows) {
            "NUL".into()
        } else {
            "/dev/null".into()
        },
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "StrictHostKeyChecking=yes".into(),
        "-o".into(),
        "ConnectTimeout=10".into(),
        "-o".into(),
        "ServerAliveInterval=5".into(),
        "-o".into(),
        "ServerAliveCountMax=2".into(),
        "-i".into(),
        identity.to_string_lossy().into_owned(),
    ];
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let uid = unsafe { libc::getuid() };
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let control = runtime.join(format!("blockcontext-ssh-{uid}"));
        if !control.exists() {
            std::fs::create_dir(&control)?;
            std::fs::set_permissions(&control, std::fs::Permissions::from_mode(0o700))?;
        }
        let info = std::fs::symlink_metadata(&control)?;
        if !info.is_dir() || info.uid() != uid || info.permissions().mode() & 0o077 != 0 {
            anyhow::bail!("Private SSH control directory required.")
        }
        args.splice(
            0..0,
            [
                "-o".into(),
                "ControlMaster=auto".into(),
                "-o".into(),
                "ControlPersist=60".into(),
                "-o".into(),
                format!("ControlPath={}", control.join("bridge").display()),
            ],
        );
    }
    args.extend([
        host.to_owned(),
        "sudo /usr/local/bin/blockcontext-import-client-snapshot".into(),
    ]);
    let log = config
        .get("logPath")
        .map(|v| {
            v.as_str()
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .ok_or_else(|| anyhow::anyhow!("Bridge logPath must be absolute."))
        })
        .transpose()?;
    let diagnostics = std::sync::Arc::new(crate::diagnostics::Diagnostics::new(log).for_bridge());
    diagnostics.lifecycle("started", "bridge", "ssh");
    diagnostics.resource();
    let mut next_resource = std::time::Instant::now() + Duration::from_secs(60);
    let stop = async {
        #[cfg(unix)]
        {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Signal handler");
            tokio::select! { _=term.recv()=>{}, _=tokio::signal::ctrl_c()=>{} }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    };
    tokio::pin!(stop);
    let mut previous = None;
    let mut last_status = "";
    loop {
        let status = match private_read(&source, 1024 * 1024) {
            Ok(blob) => {
                let projected = {
                    let _span = diagnostics.span("bridge_project");
                    crate::diagnostics::scoped(diagnostics.clone(), || project(&blob, owner, now()))
                };
                match projected {
                    Ok(blob) => {
                        let hash = digest(std::str::from_utf8(&blob).unwrap_or(""));
                        if previous.as_ref() == Some(&hash) {
                            diagnostics.count("bridge_unchanged", 1);
                            "ready"
                        } else {
                            let _span = diagnostics.span("bridge_ssh");
                            let upload_bytes = blob.len() as u64;
                            let result = async {
                                let mut child = tokio::process::Command::new("ssh")
                                    .args(&args)
                                    .stdin(std::process::Stdio::piped())
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .kill_on_drop(true)
                                    .spawn()?;
                                let mut stdin = child.stdin.take().ok_or_else(|| {
                                    std::io::Error::other("SSH input unavailable")
                                })?;
                                tokio::io::AsyncWriteExt::write_all(&mut stdin, &blob).await?;
                                drop(stdin);
                                let result = child.wait().await?;
                                if !result.success() {
                                    return Err(std::io::Error::other("Upload unavailable"));
                                }
                                Ok(())
                            };
                            match tokio::time::timeout(Duration::from_secs(25), result).await {
                                Ok(Ok(())) => {
                                    diagnostics.count("bridge_uploads", 1);
                                    diagnostics.count("bridge_upload_bytes", upload_bytes);
                                    previous = Some(hash);
                                    "ready"
                                }
                                _ => {
                                    diagnostics.count("bridge_upload_failures", 1);
                                    "snapshot_or_upload_unavailable"
                                }
                            }
                        }
                    }
                    Err(_) => {
                        diagnostics.count("bridge_snapshot_failures", 1);
                        "snapshot_or_upload_unavailable"
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                diagnostics.count("bridge_waiting", 1);
                "waiting_for_snapshot"
            }
            Err(_) => "snapshot_or_upload_unavailable",
        };
        if status != last_status {
            println!("BlockContext client bridge: {status}");
            last_status = status;
        }
        if std::time::Instant::now() >= next_resource {
            diagnostics.resource();
            next_resource = std::time::Instant::now() + Duration::from_secs(60);
        }
        tokio::select! { _=tokio::time::sleep(Duration::from_secs(2))=>{}, _=&mut stop=>{ diagnostics.resource();diagnostics.lifecycle("stopping","bridge","ssh");return Ok(()) } }
    }
}
pub fn import_fixed() -> anyhow::Result<()> {
    let owner = std::fs::read_to_string("/etc/blockcontext/client-owner")
        .map_err(|_| anyhow::anyhow!("Snapshot import rejected."))?;
    let mut blob = Vec::new();
    std::io::stdin()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut blob)?;
    let blob = project(&blob, owner.trim(), now())
        .map_err(|_| anyhow::anyhow!("Snapshot import rejected."))?;
    let dest = Path::new("/var/lib/blockcontext/client/latest.json");
    let dir = dest.parent().unwrap();
    if std::fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink())
        || std::fs::symlink_metadata(dest).is_ok_and(|m| m.file_type().is_symlink())
    {
        anyhow::bail!("Snapshot import rejected.")
    }
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let temp = dir.join(format!(".snapshot-{}-{}", std::process::id(), now()));
    let result = (|| -> anyhow::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&temp)?;
        file.write_all(&blob)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            let user = unsafe { libc::getpwnam(c"blockcontext".as_ptr()) };
            if user.is_null() {
                anyhow::bail!("Service account unavailable.")
            }
            let user = unsafe { &*user };
            use std::os::unix::ffi::OsStrExt;
            let path = std::ffi::CString::new(dir.as_os_str().as_bytes())?;
            if unsafe { libc::chown(path.as_ptr(), user.pw_uid, user.pw_gid) } != 0
                || unsafe {
                    libc::fchown(
                        std::os::fd::AsRawFd::as_raw_fd(&file),
                        user.pw_uid,
                        user.pw_gid,
                    )
                } != 0
            {
                anyhow::bail!("Snapshot import rejected.")
            }
        }
        std::fs::rename(&temp, dest)?;
        std::fs::File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
