use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Serial {
    pub baud: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
    pub flow: String,
}
impl Default for Serial {
    fn default() -> Self {
        Self {
            baud: 115200,
            data_bits: 8,
            parity: "none".into(),
            stop_bits: 1,
            flow: "none".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Ssh {
    pub host: String,
    pub user: String,
    #[serde(default = "ssh_port")]
    pub port: u16,
    pub identity: Option<PathBuf>,
    pub known_hosts: Option<PathBuf>,
}
fn ssh_port() -> u16 {
    22
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub light: bool,
    pub show_all: bool,
    pub scrollback: usize,
    pub serial: Serial,
    pub devices: BTreeMap<String, Serial>,
    pub ssh: BTreeMap<String, Ssh>,
    pub recent: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            light: false,
            show_all: false,
            scrollback: 2000,
            serial: Serial::default(),
            devices: BTreeMap::new(),
            ssh: BTreeMap::new(),
            recent: Vec::new(),
        }
    }
}

pub fn default_path() -> Result<PathBuf> {
    Ok(directories::ProjectDirs::from("", "", "PortStudio")
        .context("No application configuration directory")?
        .config_dir()
        .join("settings.toml"))
}
impl Settings {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents)
                .with_context(|| format!("Invalid settings in {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("Reading {}", path.display())),
        }
    }
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        use std::io::Write;
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        file.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
    pub fn remember(&mut self, endpoint: &str) {
        self.recent.retain(|value| value != endpoint);
        self.recent.insert(0, endpoint.into());
        self.recent.truncate(20);
    }
}
