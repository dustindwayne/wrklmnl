use super::{config, discovery::Device, transfer};
use crate::icons::{button, Icon};
use eframe::egui;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

#[derive(Default, PartialEq)]
enum View {
    #[default]
    Closed,
    Serial,
    Transfer,
}

type RemoteList = anyhow::Result<(String, Vec<(String, bool)>)>;

#[derive(Default)]
pub struct Tools {
    view: View,
    port: String,
    baud: String,
    profile: String,
    folder: PathBuf,
    local_entries: Vec<(PathBuf, bool)>,
    local_file: Option<PathBuf>,
    remote_folder: Option<String>,
    remote_entries: Vec<(String, bool)>,
    remote_file: Option<String>,
    pending: Option<mpsc::Receiver<RemoteList>>,
    status: String,
}

impl Tools {
    pub fn is_open(&self) -> bool {
        self.view != View::Closed
    }

    pub fn open_serial(&mut self, port: &str, baud: &str) {
        self.view = View::Serial;
        self.port = port.into();
        self.baud = baud.into();
        self.status.clear();
    }

    pub fn open_transfer(&mut self, root: &Path, profile: Option<&str>) {
        self.view = View::Transfer;
        self.profile = profile.unwrap_or_default().into();
        self.reset_remote();
        self.browse_local(root.to_path_buf());
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        root: &Path,
        devices: &[Device],
        settings: Option<&config::Settings>,
    ) -> Option<Vec<String>> {
        ui.horizontal(|ui| {
            if button(ui, Icon::Back, "Back to connections").clicked() {
                self.view = View::Closed;
            }
            ui.heading(match self.view {
                View::Serial => "Serial setup",
                View::Transfer => "File transfer",
                View::Closed => "Connections",
            });
        });
        ui.add_space(8.0);
        match self.view {
            View::Serial => self.serial(ui, devices, settings),
            View::Transfer => self.sftp(ui, root, settings),
            View::Closed => None,
        }
    }

    fn serial(
        &mut self,
        ui: &mut egui::Ui,
        devices: &[Device],
        settings: Option<&config::Settings>,
    ) -> Option<Vec<String>> {
        let mut command = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.weak("Choose a detected device or enter a port manually.");
            ui.add_space(8.0);
            crate::tiles::grid(ui, devices, 215.0, |ui, device, width| {
                let detail = format!("{} · {}", device.port, device.connection_state);
                if crate::tiles::tile(
                    ui,
                    width,
                    "connection",
                    &device.name,
                    &detail,
                    self.port == device.port,
                )
                .clicked()
                {
                    self.port = device.port.clone();
                    self.baud.clear();
                }
            });
            ui.add_space(8.0);
            let configured = settings
                .map(|settings| {
                    devices
                        .iter()
                        .find(|device| device.port == self.port.trim())
                        .and_then(|device| settings.devices.get(&device.id))
                        .unwrap_or(&settings.serial)
                        .baud
                })
                .unwrap_or(115200);
            ui.add_space(14.0);
            ui.heading("Port settings");
            ui.weak("Select a device above to fill its port. You can also enter a port directly.");
            ui.add_space(6.0);
            ui.scope(|ui| {
                let width = ui.available_width().min(580.0);
                ui.spacing_mut().interact_size.y = 40.0;
                ui.spacing_mut().button_padding = egui::vec2(12.0, 10.0);
                ui.strong("Port");
                ui.add_sized(
                    [width, 40.0],
                    egui::TextEdit::singleline(&mut self.port)
                        .margin(egui::Margin::symmetric(12, 10))
                        .hint_text("COM3 or /dev/ttyUSB0"),
                );
                ui.add_space(4.0);
                ui.strong("Baud rate");
                let display = if self.baud.is_empty() {
                    format!("Profile default · {configured}")
                } else {
                    self.baud.clone()
                };
                crate::icons::combo_menu_icon(egui::ComboBox::from_id_salt("serial_baud"))
                    .width(width)
                    .selected_text(display)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.baud,
                            String::new(),
                            format!("Profile default · {configured}"),
                        );
                        for rate in [
                            9600, 19200, 38400, 57600, 74880, 115200, 230400, 460800, 921600,
                        ] {
                            ui.selectable_value(&mut self.baud, rate.to_string(), rate.to_string());
                        }
                    });
            });
            ui.add_space(14.0);
            if ui
                .add_enabled(
                    !self.port.trim().is_empty(),
                    egui::Button::new("Open serial terminal")
                        .fill(ui.visuals().selection.bg_fill)
                        .min_size(egui::vec2(178.0, 42.0)),
                )
                .clicked()
            {
                let mut args = vec!["serial".into()];
                if !self.baud.is_empty() {
                    args.extend(["--baud".into(), self.baud.clone()]);
                }
                args.extend(["--".into(), self.port.trim().into()]);
                command = Some(args);
                self.view = View::Closed;
            }
        });
        command
    }

    fn sftp(
        &mut self,
        ui: &mut egui::Ui,
        root: &Path,
        settings: Option<&config::Settings>,
    ) -> Option<Vec<String>> {
        if let Some(outcome) = self.pending.as_ref().map(|job| job.try_recv()) {
            match outcome {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok((folder, items)) => {
                            self.remote_folder = Some(folder);
                            self.remote_entries = items;
                            self.remote_file = None;
                            self.status.clear();
                        }
                        Err(error) => {
                            self.status = format!("Could not open remote folder: {error:#}")
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.status = "Remote browser stopped".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let profiles = settings.map(|s| &s.ssh);
        if profiles.is_none_or(|profiles| profiles.is_empty()) {
            ui.label("Save an SSH connection before starting a transfer.");
            ui.weak("Use Manual connection → SSH authentication and saving.");
            return None;
        }
        ui.horizontal_wrapped(|ui| {
            ui.strong("SSH connection");
            let previous = self.profile.clone();
            crate::icons::combo_menu_icon(egui::ComboBox::from_id_salt("sftp_profile"))
                .selected_text(if self.profile.is_empty() {
                    "Choose saved profile"
                } else {
                    &self.profile
                })
                .show_ui(ui, |ui| {
                    if let Some(profiles) = profiles {
                        for name in profiles.keys() {
                            ui.selectable_value(&mut self.profile, name.clone(), name);
                        }
                    }
                });
            if previous != self.profile {
                self.reset_remote();
            }
            let profile = profiles.and_then(|profiles| profiles.get(&self.profile));
            if let Some(profile) = profile {
                if ui
                    .add_enabled(
                        self.pending.is_none(),
                        egui::Button::new(if self.remote_folder.is_some() {
                            "Refresh remote"
                        } else {
                            "Browse remote"
                        }),
                    )
                    .clicked()
                {
                    let path = self.remote_folder.clone().unwrap_or_else(|| ".".into());
                    self.browse_remote(ui.ctx(), profile, path);
                }
            }
        });
        let profile = profiles
            .and_then(|profiles| profiles.get(&self.profile))
            .cloned();
        if self.pending.is_some() {
            ui.weak("Reading remote files…");
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        ui.add_space(8.0);
        ui.columns(2, |columns| {
            self.local_column(&mut columns[0], root);
            self.remote_column(&mut columns[1], profile.as_ref());
        });
        ui.add_space(10.0);
        let mut command = None;
        ui.horizontal_wrapped(|ui| {
            let available =
                self.pending.is_none() && profile.is_some() && self.remote_folder.is_some();
            if ui
                .add_enabled(
                    available && self.local_file.is_some(),
                    egui::Button::new("Upload selected →"),
                )
                .clicked()
            {
                if let (Some(folder), Some(file)) = (&self.remote_folder, &self.local_file) {
                    if let Some(name) = file.file_name().and_then(|name| name.to_str()) {
                        command =
                            Some(self.transfer_args("upload", file, &remote_join(folder, name)));
                    } else {
                        self.status = "This local file name cannot be transferred".into();
                    }
                }
            }
            if ui
                .add_enabled(
                    available && self.remote_file.is_some(),
                    egui::Button::new("← Download selected"),
                )
                .clicked()
            {
                if let (Some(folder), Some(name)) = (&self.remote_folder, &self.remote_file) {
                    match crate::file_properties::validate_name(name) {
                        Ok(()) => {
                            command = Some(self.transfer_args(
                                "download",
                                &self.folder.join(name),
                                &remote_join(folder, name),
                            ))
                        }
                        Err(error) => self.status = error.to_string(),
                    }
                }
            }
        });
        ui.weak("Transfers open a progress tab and can resume after a connection drop.");
        command
    }

    fn transfer_args(&self, operation: &str, local: &Path, remote: &str) -> Vec<String> {
        let local = local.to_string_lossy().into_owned();
        let paths = if operation == "upload" {
            [local, remote.into()]
        } else {
            [remote.into(), local]
        };
        [
            operation.into(),
            format!("--profile={}", self.profile),
            "--".into(),
        ]
        .into_iter()
        .chain(paths)
        .collect()
    }

    fn local_column(&mut self, ui: &mut egui::Ui, root: &Path) {
        ui.heading("Local");
        ui.horizontal(|ui| {
            if button(ui, Icon::Home, "Workspace root").clicked() {
                self.browse_local(root.to_path_buf());
            }
            if let Some(parent) = self.folder.parent().map(Path::to_path_buf) {
                if button(ui, Icon::Up, "Parent folder").clicked() {
                    self.browse_local(parent);
                }
            }
            if button(ui, Icon::Refresh, "Refresh local files").clicked() {
                self.browse_local(self.folder.clone());
            }
        });
        ui.add(egui::Label::new(self.folder.display().to_string()).truncate())
            .on_hover_text(self.folder.display().to_string());
        let mut browse = None;
        let height = (ui.available_height() - 90.0).clamp(160.0, 500.0);
        egui::ScrollArea::vertical()
            .id_salt("sftp_local")
            .max_height(height)
            .show(ui, |ui| {
                for (path, directory) in &self.local_entries {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if file_row(
                        ui,
                        &name,
                        *directory,
                        self.local_file.as_ref() == Some(path),
                    )
                    .clicked()
                    {
                        if *directory {
                            browse = Some(path.clone());
                        } else {
                            self.local_file = Some(path.clone());
                        }
                    }
                }
            });
        if let Some(path) = browse {
            self.browse_local(path);
        }
    }

    fn remote_column(&mut self, ui: &mut egui::Ui, profile: Option<&config::Ssh>) {
        ui.heading("Remote");
        let Some(folder) = self.remote_folder.as_deref() else {
            ui.weak("Choose a saved connection and select Browse remote.");
            return;
        };
        let mut browse = None;
        if ui
            .add_enabled_ui(self.pending.is_none() && folder != "/", |ui| {
                button(ui, Icon::Up, "Remote parent folder")
            })
            .inner
            .clicked()
        {
            browse = Some(remote_join(folder, ".."));
        }
        ui.add(egui::Label::new(folder).truncate())
            .on_hover_text(folder);
        let height = (ui.available_height() - 90.0).clamp(160.0, 500.0);
        egui::ScrollArea::vertical()
            .id_salt("sftp_remote")
            .max_height(height)
            .show(ui, |ui| {
                for (name, directory) in &self.remote_entries {
                    if file_row(
                        ui,
                        name,
                        *directory,
                        self.remote_file.as_ref() == Some(name),
                    )
                    .clicked()
                    {
                        if *directory {
                            browse = Some(remote_join(folder, name));
                        } else {
                            self.remote_file = Some(name.clone());
                        }
                    }
                }
            });
        if let (None, Some(profile), Some(path)) = (&self.pending, profile, browse) {
            self.browse_remote(ui.ctx(), profile, path);
        }
    }

    fn reset_remote(&mut self) {
        self.pending = None;
        self.remote_folder = None;
        self.remote_entries.clear();
        self.remote_file = None;
        self.status.clear();
    }

    fn browse_local(&mut self, folder: PathBuf) {
        let result = std::fs::read_dir(&folder).map(|entries| {
            let mut entries: Vec<_> = entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let kind = entry.file_type().ok()?;
                    (kind.is_dir() || kind.is_file()).then(|| (entry.path(), kind.is_dir()))
                })
                .collect();
            entries.sort_by_key(|(path, directory)| {
                (
                    !directory,
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase(),
                )
            });
            entries
        });
        match result {
            Ok(entries) => {
                self.folder = folder;
                self.local_entries = entries;
                self.local_file = None;
                self.status.clear();
            }
            Err(error) => self.status = format!("Could not open local folder: {error}"),
        }
    }

    fn browse_remote(&mut self, ctx: &egui::Context, profile: &config::Ssh, path: String) {
        let profile = profile.clone();
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.status.clear();
        std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|runtime| runtime.block_on(transfer::list(&profile, &path)));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
}

fn file_row(ui: &mut egui::Ui, name: &str, directory: bool, selected: bool) -> egui::Response {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 18.0), egui::Sense::hover());
        let icon = crate::launcher::file_kind(Path::new(name), directory).0;
        crate::tiles::paint_icon(
            ui.painter(),
            icon,
            rect.min,
            0.75,
            ui.visuals().text_color(),
        );
        ui.add(
            egui::Button::new(egui::RichText::new(name))
                .selected(selected)
                .truncate(),
        )
        .on_hover_text(name)
    })
    .inner
}

fn remote_join(folder: &str, name: &str) -> String {
    format!("{}/{name}", folder.trim_end_matches('/'))
}
