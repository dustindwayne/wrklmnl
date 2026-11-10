use anyhow::Result;
use serde::Serialize;
use serialport::SerialPortType;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub port: String,
    pub category: &'static str,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub driver: Option<String>,
    pub driver_version: Option<String>,
    pub hardware_path: Option<String>,
    pub connection_state: &'static str,
    pub capabilities: [&'static str; 1],
}

pub fn scan() -> Result<Vec<Device>> {
    let mut ports = BTreeMap::new();
    for info in serialport::available_ports()? {
        let port = info.port_name;
        let mut device = Device {
            id: format!("port:{port}"),
            name: port.clone(),
            port: port.clone(),
            category: "generic_serial",
            vid: None,
            pid: None,
            manufacturer: None,
            product: None,
            serial_number: None,
            driver: None,
            driver_version: None,
            hardware_path: None,
            connection_state: "available",
            capabilities: ["serial"],
        };
        match info.port_type {
            SerialPortType::UsbPort(usb) => {
                device.vid = Some(usb.vid);
                device.pid = Some(usb.pid);
                device.manufacturer = usb.manufacturer;
                device.product = usb.product;
                device.serial_number = usb.serial_number.filter(|serial| !serial.is_empty());
                device.name = device
                    .product
                    .clone()
                    .unwrap_or_else(|| format!("USB serial ({port})"));
                let name = device.name.to_lowercase();
                device.category = if name.contains("arduino") || name.contains("pico") {
                    "dev_board"
                } else if name.contains("j-link") || name.contains("st-link") {
                    "programmer"
                } else if matches!(usb.vid, 0x0403 | 0x10c4 | 0x1a86 | 0x067b) {
                    "usb_uart"
                } else {
                    "serial_adapter"
                };
                if let Some(serial) = &device.serial_number {
                    device.id = format!("usb:{:04x}:{:04x}:{serial}", usb.vid, usb.pid);
                }
            }
            SerialPortType::BluetoothPort => device.category = "serial_adapter",
            _ => {}
        }
        #[cfg(target_os = "linux")]
        {
            let tty = std::path::Path::new(&port).file_name().unwrap_or_default();
            let path = std::path::Path::new("/sys/class/tty")
                .join(tty)
                .join("device");
            device.hardware_path = path.canonicalize().ok().map(|p| p.display().to_string());
            device.driver = path
                .join("driver")
                .canonicalize()
                .ok()
                .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()));
        }
        ports.insert(port, device);
    }
    let mut devices: Vec<_> = ports.into_values().collect();
    let mut counts = BTreeMap::new();
    for device in &devices {
        *counts.entry(device.id.clone()).or_insert(0) += 1;
    }
    for device in &mut devices {
        if counts[&device.id] > 1 {
            device.id = format!("{}:{}", device.id, device.port);
        }
    }
    devices.sort_by_key(|d| match d.category {
        "usb_uart" => 0,
        "dev_board" => 1,
        "programmer" => 2,
        "serial_adapter" => 3,
        _ => 4,
    });
    #[cfg(windows)]
    super::windows_metadata::enrich(&mut devices);
    Ok(devices)
}

pub async fn snapshot() -> Result<Vec<Device>> {
    tokio::task::spawn_blocking(scan).await?
}
