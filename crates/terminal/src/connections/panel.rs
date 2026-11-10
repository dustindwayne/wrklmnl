use super::{
    config,
    discovery::{self, Device},
};
use eframe::egui;
use std::{sync::mpsc, time::Duration};

#[derive(Default)]
pub struct Panel {
    scan: Option<mpsc::Receiver<anyhow::Result<Vec<Device>>>>,
    devices: Vec<Device>,
    saved: Option<config::Settings>,
    selected_ssh: Option<String>,
    error: String,
    profile_status: String,
    selected: Option<String>,
    port: String,
    baud: String,
    host: String,
    user: String,
    network_port: String,
    focus_endpoint: bool,
    identity: String,
    profile: String,
    tools: super::tools_view::Tools,
    telnet: bool,
    network_open: bool,
    network_error: String,
}

impl Panel {
    #[cfg(feature = "capture")]
    pub fn capture_view(&mut self, view: &str, root: &std::path::Path) {
        match view {
            "ssh" => self.quick_connect(false, "operator@router.example".into()),
            "telnet" => self.quick_connect(true, "switch.example".into()),
            "serial" => self.tools.open_serial("COM4", ""),
            "transfer" => self.tools.open_transfer(root, Some("Lab router")),
            _ => {}
        }
    }

    fn save_profile(&self) -> anyhow::Result<()> {
        let path = config::default_path()?;
        let (host, user, port) = self.connection_details()?;
        let key = self.profile.trim().to_owned();
        config::Settings::update(&path, |settings| {
            let known_hosts = settings
                .ssh
                .get(&key)
                .and_then(|profile| profile.known_hosts.clone());
            settings.ssh.insert(
                key,
                config::Ssh {
                    host: host.into(),
                    user: user.unwrap_or_default().into(),
                    port,
                    identity: (!self.identity.trim().is_empty())
                        .then(|| self.identity.trim().into()),
                    known_hosts,
                },
            );
        })
    }

    fn update(&mut self, ui: &egui::Ui) {
        #[cfg(feature = "capture")]
        if std::env::var_os("DTERM_CAPTURE_SAMPLE").is_some()
            && std::env::var_os("TERMINAL_CAPTURE_TO").is_some()
        {
            if self.saved.is_none() {
                self.devices = [
                    ("COM4", "USB serial adapter", "usb_uart"),
                    ("COM6", "Development board", "dev_board"),
                ]
                .into_iter()
                .map(|(port, name, category)| Device {
                    id: port.into(),
                    name: name.into(),
                    port: port.into(),
                    category,
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
                })
                .collect();
                let mut settings = config::Settings::default();
                settings.ssh.insert(
                    "Lab router".into(),
                    config::Ssh {
                        host: "router.example".into(),
                        user: "operator".into(),
                        port: 22,
                        identity: None,
                        known_hosts: None,
                    },
                );
                self.saved = Some(settings);
            }
            return;
        }
        if self.saved.is_none() {
            self.saved = config::default_path()
                .and_then(|path| config::Settings::load(&path))
                .ok();
        }
        if self.scan.is_none() {
            let (tx, rx) = mpsc::sync_channel(1);
            let ctx = ui.ctx().clone();
            std::thread::spawn(move || loop {
                if tx.send(discovery::scan()).is_err() {
                    break;
                }
                ctx.request_repaint();
                std::thread::sleep(Duration::from_millis(850));
            });
            self.scan = Some(rx);
        }
        if let Some(Ok(result)) = self.scan.as_ref().map(|scan| scan.try_recv()) {
            match result {
                Ok(mut devices) => {
                    for old in &self.devices {
                        if !devices.iter().any(|device| device.id == old.id) {
                            let mut missing = old.clone();
                            missing.connection_state = "disconnected";
                            devices.push(missing);
                        }
                    }
                    self.devices = devices;
                    self.error.clear();
                }
                Err(error) => self.error = error.to_string(),
            }
            match config::default_path().and_then(|path| config::Settings::load(&path)) {
                Ok(settings) => self.saved = Some(settings),
                Err(error) => self.error = format!("Could not read saved connections: {error:#}"),
            }
        }
    }

    pub fn overview(&mut self, ui: &mut egui::Ui) -> bool {
        self.update(ui);
        ui.heading("Connections");
        let available = self
            .devices
            .iter()
            .filter(|device| device.connection_state == "available")
            .count();
        let saved = self.saved.as_ref().map_or(0, |settings| settings.ssh.len());
        ui.weak(format!(
            "{available} serial available · {saved} saved SSH profiles"
        ));
        for device in self.devices.iter().take(4) {
            ui.label(format!(
                "{} · {} · {}",
                device.name, device.port, device.connection_state
            ));
        }
        if let Some(settings) = &self.saved {
            for (name, profile) in settings.ssh.iter().take(3) {
                ui.label(format!(
                    "{name} · SSH / SFTP · {}:{}",
                    profile.host, profile.port
                ));
            }
        }
        crate::widgets::button(ui, "Open Connections").clicked()
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &std::path::Path) -> Option<Vec<String>> {
        self.update(ui);
        if self.network_open {
            return self.manual_connection(ui);
        }
        if self.tools.is_open() {
            let settings = self.saved.as_ref();
            return self.tools.show(ui, root, &self.devices, settings);
        }
        use crate::icons::{button, Icon};
        let mut args = None;
        ui.heading("Connections");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if button(ui, Icon::Plus, "Manual connection").clicked() {
                self.quick_connect(false, String::new());
            }
            if button(ui, Icon::Serial, "Serial setup").clicked() {
                self.tools.open_serial(&self.port, &self.baud);
            }
            if button(ui, Icon::Transfer, "File transfer").clicked() {
                self.tools.open_transfer(root, self.selected_ssh.as_deref());
            }
            ui.separator();
            crate::icons::menu(ui, "Connection menu", |ui| {
                ui.weak("QUICK CONNECT");
                for (label, telnet) in [("SSH", false), ("Telnet", true)] {
                    if ui.button(label).clicked() {
                        self.quick_connect(telnet, String::new());
                        ui.close_menu();
                    }
                }
                if let Some(settings) = &self.saved {
                    if !settings.ssh.is_empty() {
                        ui.separator();
                        ui.weak("SAVED SSH");
                        for name in settings.ssh.keys().take(8) {
                            if ui.button(name).clicked() {
                                args = Some(saved_ssh_args(name));
                                ui.close_menu();
                            }
                        }
                    }
                }
                if let Some(settings) = config::default_path()
                    .and_then(|path| config::Settings::load(&path))
                    .ok()
                {
                    let mut recent = settings
                        .recent
                        .iter()
                        .filter_map(|entry| {
                            if let Some(endpoint) = entry.strip_prefix("telnet://") {
                                Some((true, endpoint))
                            } else if entry.contains('@') {
                                Some((false, entry.as_str()))
                            } else {
                                None
                            }
                        })
                        .take(6)
                        .peekable();
                    if recent.peek().is_some() {
                        ui.separator();
                        ui.weak("RECENT CONNECTIONS");
                        for (telnet, endpoint) in recent {
                            if ui
                                .button(format!(
                                    "{}  {endpoint}",
                                    if telnet { "Telnet" } else { "SSH" }
                                ))
                                .clicked()
                            {
                                self.quick_connect(telnet, recent_endpoint(endpoint));
                                ui.close_menu();
                            }
                        }
                    }
                }
            });
        });
        ui.separator();
        ui.add_space(6.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.weak("Inspect a connection with one click. Double-click to open a terminal.");
            ui.add_space(14.0);
            ui.label("Serial");
            if !self.error.is_empty() {
                ui.label(&self.error);
            }
            if self.devices.is_empty() {
                ui.weak("No serial devices detected. Enter a port to connect manually.");
            }
            crate::tiles::grid(ui, &self.devices, 216.0, |ui, device, width| {
                let detail = format!("{} · {} · {}", device.port, device.category.replace('_', " "), device.connection_state);
                let response = crate::tiles::tile(ui, width, "connection", &device.name, &detail, self.selected.as_deref() == Some(&device.id))
                    .on_hover_text(format!("{}\n{detail}", device.name));
                if response.clicked() {
                    self.selected = Some(device.id.clone());
                    self.selected_ssh = None;
                    if self.port != device.port { self.baud.clear(); }
                    self.port = device.port.clone();
                }
                if device.connection_state == "available" && (response.double_clicked() || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))) {
                    args = Some(serial_args(&device.port, if self.port == device.port { &self.baud } else { "" }));
                }
            });
            if let Some(device) = self.devices.iter().find(|d| Some(&d.id) == self.selected.as_ref()) {
                let settings = self.saved.as_ref();
                let serial = settings.and_then(|s| s.devices.get(&device.id).or(Some(&s.serial))).cloned().unwrap_or_default();
                ui.add_space(8.0);
                ui.strong(&device.name);
                ui.label(format!("Serial · {} · {}", device.port, device.connection_state));
                ui.label(format!("Suggested settings · {} baud · {} data bits · {} parity · {} stop bits · {} flow", serial.baud, serial.data_bits, serial.parity, serial.stop_bits, serial.flow));
                if let (Some(vid), Some(pid)) = (device.vid, device.pid) { ui.label(format!("USB {vid:04x}:{pid:04x}")); }
                for (name, value) in [
                    ("Manufacturer", &device.manufacturer), ("Product", &device.product),
                    ("Serial number", &device.serial_number), ("Driver", &device.driver),
                    ("Driver version", &device.driver_version), ("Hardware path", &device.hardware_path),
                ] {
                    if let Some(value) = value { ui.label(format!("{name} · {value}")); }
                }
                if device.driver.is_none() { ui.weak("Driver details unavailable from the operating system"); }
                if crate::widgets::button(ui, "Dismiss details").clicked() { self.selected = None; }
            }
            if let Some(settings) = &self.saved {
                let profiles: Vec<_> = settings.ssh.iter().collect();
                if !profiles.is_empty() { ui.add_space(14.0); ui.label("Saved network connections"); }
                crate::tiles::grid(ui, &profiles, 216.0, |ui, (name, profile), width| {
                    let detail = format!("SSH / SFTP · {}@{}:{}", profile.user, profile.host, profile.port);
                    let response = crate::tiles::tile(ui, width, "connection", name, &detail, self.selected_ssh.as_ref() == Some(*name)).on_hover_text(&detail);
                    if response.clicked() {
                        self.selected_ssh = Some((*name).clone());
                        self.selected = None;
                        self.profile = (*name).clone();
                    }
                    if response.double_clicked() || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                        args = Some(saved_ssh_args(name));
                    }
                });
                if let Some((name, profile)) = self.selected_ssh.as_ref().and_then(|name| settings.ssh.get_key_value(name)) {
                    ui.strong(name);
                    ui.label(format!("SSH / SFTP over TCP · {}@{}:{}", profile.user, profile.host, profile.port));
                    ui.label(format!("Authentication · {}", if profile.identity.is_some() { "Private key" } else { "SSH agent" }));
                    ui.label("Host key verification · Required");
                    ui.weak("Network connection · no device driver is associated with this profile");
                    if crate::widgets::button(ui, "Dismiss details").clicked() { self.selected_ssh = None; }
                }
            }
        });
        args
    }

    fn quick_connect(&mut self, telnet: bool, endpoint: String) {
        self.telnet = telnet;
        self.host.clear();
        self.user.clear();
        self.network_port.clear();
        if let Ok((host, user, port)) = parse_endpoint(&endpoint, telnet) {
            self.host = host.into();
            self.user = user.unwrap_or_default().into();
            self.network_port = port.to_string();
        }
        self.identity.clear();
        self.network_error.clear();
        self.profile_status.clear();
        self.network_open = true;
        self.focus_endpoint = true;
    }

    fn manual_connection(&mut self, ui: &mut egui::Ui) -> Option<Vec<String>> {
        use crate::icons::{button, Icon};
        let mut command = None;
        ui.horizontal(|ui| {
            if button(ui, Icon::Back, "Back to connections").clicked() {
                self.network_open = false;
            }
            ui.heading("Manual connection");
        });
        ui.weak("Open a remote terminal");
        ui.add_space(10.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            let wide = ui.available_width() >= 760.0;
            if wide {
                ui.columns(2, |columns| {
                    command = self.connection_editor(&mut columns[0]);
                    if command.is_none() {
                        command = self.connection_library(&mut columns[1]);
                    }
                });
            } else {
                command = self.connection_editor(ui);
                ui.add_space(14.0);
                if command.is_none() {
                    command = self.connection_library(ui);
                }
            }
        });
        if command.is_some() {
            self.network_open = false;
        }
        command
    }

    fn connection_editor(&mut self, ui: &mut egui::Ui) -> Option<Vec<String>> {
        ui.label(egui::RichText::new("Protocol").strong());
        let width = ((ui.available_width() - ui.spacing().item_spacing.x) * 0.5).max(80.0);
        ui.horizontal(|ui| {
            for (title, detail, telnet) in [
                ("SSH", "Encrypted shell", false),
                ("Telnet", "Legacy cleartext", true),
            ] {
                if crate::tiles::tile(
                    ui,
                    width,
                    "connection",
                    title,
                    detail,
                    self.telnet == telnet,
                )
                .clicked()
                    && self.telnet != telnet
                {
                    self.telnet = telnet;
                    self.network_port.clear();
                    self.network_error.clear();
                    self.focus_endpoint = true;
                }
            }
        });
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Destination").strong());
        let mut enter = false;
        ui.horizontal(|ui| {
            let host_width = (ui.available_width() - 92.0).max(100.0);
            ui.vertical(|ui| {
                ui.weak("Host");
                let host = ui.add(
                    egui::TextEdit::singleline(&mut self.host)
                        .hint_text("Hostname or IP address")
                        .desired_width(host_width),
                );
                if self.focus_endpoint {
                    host.request_focus();
                    self.focus_endpoint = false;
                }
                enter |= host.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            });
            ui.vertical(|ui| {
                ui.weak("Port");
                let port = ui.add(
                    egui::TextEdit::singleline(&mut self.network_port)
                        .hint_text(if self.telnet { "23" } else { "22" })
                        .desired_width(64.0),
                );
                enter |= port.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            });
        });
        if self.telnet {
            ui.weak("Telnet sends session data in cleartext.");
        } else {
            ui.weak("Username");
            let user = ui.add(
                egui::TextEdit::singleline(&mut self.user)
                    .hint_text("Remote account")
                    .desired_width(200.0),
            );
            enter |= user.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.vertical(|ui| {
                ui.strong("SSH authentication and saving");
                ui.weak("The SSH agent is used when the key path is empty.");
                ui.label("Private key");
                ui.add(
                    egui::TextEdit::singleline(&mut self.identity)
                        .hint_text("Optional key file path")
                        .desired_width(240.0),
                );
                ui.horizontal_wrapped(|ui| {
                    field(ui, "Profile name", &mut self.profile);
                    if ui
                        .add_enabled(
                            !self.profile.trim().is_empty(),
                            egui::Button::new("Save SSH profile"),
                        )
                        .clicked()
                    {
                        match self.save_profile() {
                            Ok(()) => {
                                self.saved = None;
                                self.profile_status = "SSH profile saved".into();
                                self.network_error.clear();
                            }
                            Err(error) => self.network_error = error.to_string(),
                        }
                    }
                });
                if !self.profile_status.is_empty() {
                    ui.weak(&self.profile_status);
                }
            });
        }
        if !self.network_error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.network_error);
        }
        ui.add_space(10.0);
        let accent = ui.visuals().hyperlink_color;
        let start = ui.add(
            egui::Button::new(
                egui::RichText::new(if self.telnet {
                    "Open Telnet session"
                } else {
                    "Open SSH session"
                })
                .color(accent)
                .strong(),
            )
            .fill(ui.visuals().selection.bg_fill)
            .min_size(egui::vec2(155.0, 35.0)),
        );
        if start.clicked() || enter {
            match self.network_args() {
                Ok(command) => return Some(command),
                Err(error) => self.network_error = error.to_string(),
            }
        }
        None
    }

    fn connection_library(&mut self, ui: &mut egui::Ui) -> Option<Vec<String>> {
        let tint = ui.visuals().extreme_bg_color;
        let mut result = None;
        let mut reuse = None;
        egui::Frame::NONE
            .fill(tint)
            .inner_margin(16)
            .show(ui, |ui| {
                ui.heading("Connection library");
                ui.weak("Saved profiles and recent destinations");
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Saved SSH").strong());
                let saved = self.saved.as_ref().map(|settings| &settings.ssh);
                if saved.is_none_or(|profiles| profiles.is_empty()) {
                    ui.weak("No saved SSH profiles");
                }
                if let Some(saved) = saved {
                    for (name, profile) in saved.iter().take(8) {
                        ui.horizontal_wrapped(|ui| {
                            if crate::widgets::button(ui, name)
                                .on_hover_text("Open saved SSH profile")
                                .clicked()
                            {
                                result = Some(saved_ssh_args(name));
                            }
                            ui.weak(format!("{}:{}", profile.host, profile.port));
                        });
                    }
                }
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Recent").strong());
                let mut count = 0;
                if let Ok(settings) =
                    config::default_path().and_then(|path| config::Settings::load(&path))
                {
                    for item in &settings.recent {
                        let (telnet, endpoint) =
                            if let Some(endpoint) = item.strip_prefix("telnet://") {
                                (true, endpoint)
                            } else if item.contains('@') {
                                (false, item.as_str())
                            } else {
                                continue;
                            };
                        count += 1;
                        if crate::widgets::button(
                            ui,
                            format!("{}  {endpoint}", if telnet { "Telnet" } else { "SSH" }),
                        )
                        .on_hover_text("Fill the destination fields")
                        .clicked()
                        {
                            reuse = Some((telnet, recent_endpoint(endpoint)));
                        }
                        if count == 6 {
                            break;
                        }
                    }
                }
                if count == 0 {
                    ui.weak("Successful SSH and Telnet sessions appear here.");
                }
            });
        if let Some((telnet, endpoint)) = reuse {
            self.quick_connect(telnet, endpoint);
        }
        result
    }

    fn connection_details(&self) -> anyhow::Result<(String, Option<String>, u16)> {
        let host = self.host.trim();
        let host = if host.starts_with('[') && host.ends_with(']') {
            &host[1..host.len() - 1]
        } else {
            host
        };
        anyhow::ensure!(
            !host.is_empty()
                && !host.chars().any(char::is_whitespace)
                && !host.contains(['/', '@', '[', ']']),
            "Enter a hostname or IP address"
        );
        if host.contains(':') {
            host.parse::<std::net::Ipv6Addr>()
                .map_err(|_| anyhow::anyhow!("Enter the port in the separate Port field"))?;
        }
        let port = if self.network_port.trim().is_empty() {
            if self.telnet {
                23
            } else {
                22
            }
        } else {
            self.network_port
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|port| *port > 0)
                .ok_or_else(|| anyhow::anyhow!("Enter a port from 1 to 65535"))?
        };
        let user = self.user.trim();
        if !self.telnet {
            anyhow::ensure!(
                !user.is_empty() && !user.contains('@') && !user.chars().any(char::is_whitespace),
                "Enter the remote SSH username"
            );
        }
        Ok((host.into(), (!self.telnet).then(|| user.into()), port))
    }

    fn network_args(&self) -> anyhow::Result<Vec<String>> {
        let (host, user, port) = self.connection_details()?;
        if self.telnet {
            return Ok(vec![
                "telnet".into(),
                "--port".into(),
                port.to_string(),
                "--".into(),
                host,
            ]);
        }
        let user = user.unwrap_or_default();
        let mut args = vec![
            "ssh".into(),
            format!("--host={host}"),
            format!("--user={user}"),
            format!("--port={port}"),
        ];
        if !self.identity.trim().is_empty() {
            args.push(format!("--identity={}", self.identity.trim()));
        }
        Ok(args)
    }
}

fn saved_ssh_args(name: &str) -> Vec<String> {
    vec!["ssh".into(), format!("--profile={name}")]
}

// Stored history always includes a port, including entries written before IPv6 brackets.
fn recent_endpoint(endpoint: &str) -> String {
    let (user, address) = endpoint
        .rsplit_once('@')
        .map_or(("", endpoint), |(user, address)| (user, address));
    if let Some((host, port)) = address.rsplit_once(':') {
        if host.parse::<std::net::Ipv6Addr>().is_ok() && port.parse::<u16>().is_ok() {
            return if user.is_empty() {
                format!("[{host}]:{port}")
            } else {
                format!("{user}@[{host}]:{port}")
            };
        }
    }
    endpoint.into()
}

fn parse_endpoint(input: &str, telnet: bool) -> anyhow::Result<(&str, Option<&str>, u16)> {
    let input = input.trim();
    anyhow::ensure!(
        !input.is_empty() && !input.chars().any(char::is_whitespace),
        "Enter a connection address without spaces"
    );
    let (address, user) = if telnet {
        (input, None)
    } else {
        let (user, address) = input
            .rsplit_once('@')
            .ok_or_else(|| anyhow::anyhow!("Use user@host for SSH"))?;
        anyhow::ensure!(
            !user.is_empty() && !user.contains('@'),
            "Enter the SSH username before @"
        );
        (address, Some(user))
    };
    let (host, port) = if let Some(bracketed) = address.strip_prefix('[') {
        let (host, suffix) = bracketed
            .split_once(']')
            .ok_or_else(|| anyhow::anyhow!("Close the IPv6 address with ]"))?;
        let port = if suffix.is_empty() {
            None
        } else {
            Some(
                suffix
                    .strip_prefix(':')
                    .ok_or_else(|| anyhow::anyhow!("Use [IPv6]:port"))?,
            )
        };
        (host, port)
    } else if address.matches(':').count() == 1 {
        let (host, port) = address.split_once(':').unwrap();
        (host, Some(port))
    } else {
        (address, None)
    };
    anyhow::ensure!(
        !host.is_empty() && !host.contains(['/', '@', '[', ']']),
        "Enter a hostname or IP address"
    );
    if host.contains(':') {
        host.parse::<std::net::Ipv6Addr>()
            .map_err(|_| anyhow::anyhow!("Use [IPv6]:port for an IPv6 address with a port"))?;
    }
    let port = match port {
        Some(port) => port
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
            .ok_or_else(|| anyhow::anyhow!("Enter a port from 1 to 65535"))?,
        None => {
            if telnet {
                23
            } else {
                22
            }
        }
    };
    Ok((host, user, port))
}

#[cfg(test)]
mod tests {
    use super::{parse_endpoint, recent_endpoint, saved_ssh_args};

    #[test]
    fn profile_names_are_passed_as_option_values() {
        assert_eq!(saved_ssh_args("-lab"), ["ssh", "--profile=-lab"]);
    }

    #[test]
    fn quick_connect_addresses_use_protocol_defaults_and_accept_ipv6() {
        assert_eq!(
            parse_endpoint("admin@router", false).unwrap(),
            ("router", Some("admin"), 22)
        );
        assert_eq!(
            parse_endpoint("[::1]:2323", true).unwrap(),
            ("::1", None, 2323)
        );
        assert_eq!(
            parse_endpoint("router", true).unwrap(),
            ("router", None, 23)
        );
        assert_eq!(recent_endpoint("admin@::1:2222"), "admin@[::1]:2222");
        assert!(parse_endpoint("router", false).is_err());
        assert!(parse_endpoint("admin@router:0", false).is_err());
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(label)
            .desired_width(150.0),
    );
}

fn serial_args(port: &str, baud: &str) -> Vec<String> {
    let mut command = vec!["serial".into(), port.into()];
    if !baud.trim().is_empty() {
        command.extend(["--baud".into(), baud.trim().into()]);
    }
    command
}
