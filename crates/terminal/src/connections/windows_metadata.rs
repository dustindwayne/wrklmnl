use super::discovery::Device;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    os::windows::process::CommandExt,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Default)]
struct Cache {
    ports: Vec<String>,
    updated: Option<Instant>,
    pending: bool,
    rows: BTreeMap<String, Metadata>,
}

#[derive(Deserialize)]
struct Metadata {
    port: String,
    name: Option<String>,
    manufacturer: Option<String>,
    driver: Option<String>,
    version: Option<String>,
    path: Option<String>,
}

pub fn enrich(devices: &mut [Device]) {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let Ok(mut state) = cache.lock() else {
        return;
    };
    let mut ports: Vec<_> = devices.iter().map(|device| device.port.clone()).collect();
    ports.sort();
    if ports.is_empty() {
        state.rows.clear();
        state.ports.clear();
        return;
    }
    let changed = state.ports != ports;
    if changed {
        state.rows.clear();
    }
    if !state.pending
        && (changed
            || state
                .updated
                .is_none_or(|time| time.elapsed() > Duration::from_secs(30)))
    {
        state.pending = true;
        state.ports = ports.clone();
        std::thread::spawn(move || {
            let rows = query();
            if let Ok(mut state) = cache.lock() {
                state.pending = false;
                if state.ports == ports {
                    if let Some(rows) = rows {
                        state.rows = rows
                            .into_iter()
                            .map(|row| (row.port.clone(), row))
                            .collect();
                    }
                    state.updated = Some(Instant::now());
                }
            }
        });
    }
    for device in devices {
        if let Some(row) = state.rows.get(&device.port) {
            if let Some(name) = &row.name {
                device.name.clone_from(name);
            }
            if device.manufacturer.is_none() {
                device.manufacturer.clone_from(&row.manufacturer);
            }
            device.driver.clone_from(&row.driver);
            device.driver_version.clone_from(&row.version);
            device.hardware_path.clone_from(&row.path);
        }
    }
}

fn query() -> Option<Vec<Metadata>> {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$ports = @(Get-CimInstance Win32_PnPEntity -Filter "Name LIKE '%(COM%)'")
$drivers = @{}
if ($ports.Count -gt 0) {
    foreach ($driver in Get-CimInstance Win32_PnPSignedDriver) { if ($driver.DeviceID) { $drivers[$driver.DeviceID] = $driver } }
}
$rows = @(foreach ($device in $ports) {
    if ($device.Name -match '^(.*?)\s*\((COM[0-9]+)\)\s*$') {
        $name = $Matches[1]
        $port = $Matches[2]
        $driver = $drivers[$device.PNPDeviceID]
        $label = (@($device.Service, $driver.InfName) | Where-Object { $_ }) -join ' / '
        @{
            port = $port
            name = $name
            manufacturer = $device.Manufacturer
            driver = if ($label) { $label } else { $null }
            version = $driver.DriverVersion
            path = $device.PNPDeviceID
        }
    }
})
ConvertTo-Json -InputObject $rows -Compress
"#;
    let powershell = std::path::PathBuf::from(std::env::var_os("SystemRoot")?)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let output = std::process::Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .creation_flags(0x08000000)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}
