use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    pub path: PathBuf,
    pub shell: String,
    pub opened: u64,
    #[serde(default)]
    pub args: Vec<String>,
}
#[derive(Default, Serialize, Deserialize)]
pub struct History {
    pub locations: Vec<Location>,
    #[serde(default)]
    removed: Vec<Location>,
}
impl History {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.lock_shared()?;
        let result = Self::read_unlocked(path);
        lock.unlock()?;
        result
    }
    fn read_unlocked(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    #[cfg(test)]
    pub fn record(&mut self, path: &Path, shell: &str) {
        self.record_with_args(path, shell, &[]);
    }
    pub fn record_with_args(&mut self, path: &Path, shell: &str, args: &[String]) {
        self.locations
            .retain(|x| x.path != path || x.shell != shell || x.args != args);
        self.locations.insert(
            0,
            Location {
                path: path.into(),
                shell: shell.into(),
                opened: (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64)
                    .max(
                        self.removed
                            .iter()
                            .filter(|x| x.path == path && x.shell == shell && x.args == args)
                            .map(|x| x.opened.saturating_add(1))
                            .max()
                            .unwrap_or(0),
                    ),
                args: args.into(),
            },
        );
        self.locations.truncate(12);
    }
    pub fn remove(&mut self, location: &Location) {
        let mut removed = location.clone();
        removed.opened = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        removed.opened = removed.opened.max(location.opened.saturating_add(1));
        self.removed.push(removed);
        self.locations.retain(|x| {
            x.path != location.path || x.shell != location.shell || x.args != location.args
        });
    }
    pub fn save(&mut self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))?;
        lock.lock()?;
        let mut merged = Self::read_unlocked(path)?;
        merged.locations.extend(self.locations.clone());
        merged.removed.extend(self.removed.clone());
        merged.removed.sort_by(|a, b| b.opened.cmp(&a.opened));
        let mut removed_keys = std::collections::HashSet::new();
        merged
            .removed
            .retain(|x| removed_keys.insert((x.path.clone(), x.shell.clone(), x.args.clone())));
        merged.removed.truncate(128);
        merged.locations.retain(|x| {
            !merged.removed.iter().any(|r| {
                r.path == x.path && r.shell == x.shell && r.args == x.args && r.opened >= x.opened
            })
        });
        merged.locations.sort_by(|a, b| b.opened.cmp(&a.opened));
        let mut seen = std::collections::HashSet::new();
        merged
            .locations
            .retain(|x| seen.insert((x.path.clone(), x.shell.clone(), x.args.clone())));
        merged.locations.truncate(12);
        crate::persistence::atomic_write(path, &serde_json::to_vec_pretty(&merged)?)?;
        *self = merged;
        lock.unlock()?;
        Ok(())
    }
}
pub fn default_path() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
    };
    let base = base.unwrap_or_else(|| PathBuf::from("."));
    let preferred = base.join("dterm/recent.json");
    let legacy = base.join("terminal-ui/recent.json");
    if !preferred.exists() && legacy.exists() {
        legacy
    } else {
        preferred
    }
}
pub fn display_path(path: &Path) -> String {
    let text = path.display().to_string();
    if let Some(rest) = text.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else {
        text.strip_prefix("\\\\?\\").unwrap_or(&text).to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deduplicate_and_bound() {
        let mut h = History::default();
        for i in 0..20 {
            h.record(Path::new(&format!("folder{i}")), "sh");
        }
        h.record(Path::new("folder19"), "sh");
        assert_eq!(h.locations.len(), 12);
        assert_eq!(h.locations[0].path, Path::new("folder19"));
    }
    #[test]
    fn concurrent_history_is_merged() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/history-check");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.json", std::process::id()));
        let handles: Vec<_> = (0..6)
            .map(|i| {
                let p = path.clone();
                std::thread::spawn(move || {
                    let mut h = History::default();
                    h.record_with_args(Path::new(&format!("folder{i}")), "sh", &["-l".into()]);
                    h.save(&p).unwrap();
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let h = History::load(&path).unwrap();
        assert_eq!(h.locations.len(), 6);
        assert_eq!(h.locations[0].args, vec!["-l"]);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn removing_and_reopening_in_same_tick() {
        let mut h = History::default();
        h.record(Path::new("folder"), "shell");
        let old = h.locations[0].clone();
        h.remove(&old);
        assert!(h.locations.is_empty());
        assert!(h.removed[0].opened > old.opened);
        h.record(Path::new("folder"), "shell");
        assert!(h.locations[0].opened > h.removed[0].opened);
    }
}
