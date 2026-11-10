use anyhow::{ensure, Context, Result};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub folder: String,
}

impl Profile {
    pub fn program(&self) -> Result<PathBuf> {
        let entered = Path::new(self.executable.trim());
        ensure!(!entered.as_os_str().is_empty(), "Choose an executable");
        let path = if entered.is_absolute() {
            entered.to_path_buf()
        } else {
            ensure!(
                entered.file_name() == Some(entered.as_os_str()),
                "Use an absolute path or an executable on PATH"
            );
            crate::shell_tools::find(self.executable.trim())
                .context("Executable was not found on PATH")?
        };
        ensure!(is_executable(&path), "Choose a launchable executable");
        Ok(path)
    }

    fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "Enter a profile name");
        self.program()?;
        if !self.folder.trim().is_empty() {
            let folder = Path::new(self.folder.trim());
            ensure!(
                folder.is_absolute() && folder.is_dir(),
                "Choose an existing absolute startup folder"
            );
        }
        Ok(())
    }
}

pub fn is_executable(path: &Path) -> bool {
    #[cfg(windows)]
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("com"))
    {
        return false;
    }
    crate::shell_catalog::executable(path)
}

#[derive(Default)]
pub struct Shells {
    pub profiles: Vec<Profile>,
    selected: Option<usize>,
    draft: Profile,
    arguments: String,
    notice: String,
}

impl Shells {
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(profiles) => Self {
                    profiles,
                    ..Self::default()
                },
                Err(error) => Self {
                    notice: format!("Could not read saved shells: {error}"),
                    ..Self::default()
                },
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => Self {
                notice: format!("Could not read saved shells: {error}"),
                ..Self::default()
            },
        }
    }

    pub fn set_executable(&mut self, path: &Path) {
        self.draft.executable = path.display().to_string();
    }

    fn begin(&mut self, selected: Option<usize>) {
        self.selected = selected;
        self.draft = selected
            .and_then(|index| self.profiles.get(index))
            .cloned()
            .unwrap_or_default();
        self.arguments = self.draft.args.join("\n");
        self.notice.clear();
    }

    fn persist(&mut self, path: &Path, profiles: Vec<Profile>) -> Result<()> {
        if path.exists() {
            let _: Vec<Profile> = serde_json::from_slice(&std::fs::read(path)?)
                .context("Existing custom shell settings are invalid. They were preserved.")?;
        }
        crate::persistence::atomic_write(path, &serde_json::to_vec_pretty(&profiles)?)?;
        self.profiles = profiles;
        Ok(())
    }

    fn save(&mut self, path: &Path) -> Result<()> {
        let mut profile = self.draft.clone();
        profile.name = profile.name.trim().into();
        profile.executable = profile.executable.trim().into();
        profile.folder = profile.folder.trim().into();
        profile.args = if self.arguments.is_empty() {
            Vec::new()
        } else {
            self.arguments
                .split_terminator('\n')
                .map(|line| line.trim_end_matches('\r').to_owned())
                .collect()
        };
        profile.validate()?;
        ensure!(
            !self
                .profiles
                .iter()
                .enumerate()
                .any(|(index, saved)| Some(index) != self.selected
                    && saved.name.eq_ignore_ascii_case(&profile.name)),
            "A profile with this name already exists"
        );
        let mut profiles = self.profiles.clone();
        let index = self.selected.unwrap_or(profiles.len());
        if let Some(existing) = profiles.get_mut(index) {
            *existing = profile;
        } else {
            profiles.push(profile);
        }
        self.persist(path, profiles)?;
        self.begin(Some(index));
        Ok(())
    }

    pub fn show(&mut self, ui: &mut egui::Ui, path: &Path) -> bool {
        ui.heading("Custom shells");
        ui.weak("Saved shells appear in the terminal picker.");
        ui.add_space(8.0);
        let mut choose = false;
        if ui.available_width() >= 680.0 {
            ui.columns(2, |columns| {
                self.list(&mut columns[0]);
                choose = self.form(&mut columns[1], path);
            });
        } else {
            self.list(ui);
            ui.separator();
            choose = self.form(ui, path);
        }
        choose
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Saved profiles");
            if crate::icons::button(ui, crate::icons::Icon::Plus, "New shell profile").clicked() {
                self.begin(None);
            }
        });
        if self.profiles.is_empty() {
            ui.weak("No custom shells saved yet.");
        }
        let mut selected = None;
        for (index, profile) in self.profiles.iter().enumerate() {
            if ui
                .selectable_label(self.selected == Some(index), &profile.name)
                .on_hover_text(&profile.executable)
                .clicked()
            {
                selected = Some(index);
            }
        }
        if let Some(index) = selected {
            self.begin(Some(index));
        }
    }

    fn form(&mut self, ui: &mut egui::Ui, path: &Path) -> bool {
        ui.strong(if self.selected.is_some() {
            "Edit profile"
        } else {
            "New profile"
        });
        let width = ui.available_width().min(560.0);
        let input = |ui: &mut egui::Ui, label, hint, value: &mut String, width| {
            ui.label(label);
            ui.add_sized(
                [width, 38.0],
                egui::TextEdit::singleline(value)
                    .hint_text(hint)
                    .margin(egui::Margin::symmetric(10, 8)),
            );
        };
        input(
            ui,
            "Name",
            "Example: Project shell",
            &mut self.draft.name,
            width,
        );
        input(
            ui,
            "Executable",
            "Absolute path or program name",
            &mut self.draft.executable,
            width,
        );
        let choose = crate::widgets::button(ui, "Browse executable…").clicked();
        input(
            ui,
            "Startup folder",
            "Optional · use the picker’s working folder",
            &mut self.draft.folder,
            width,
        );
        ui.label("Arguments");
        ui.weak("One argument per line. Spaces stay inside the argument.");
        ui.add(
            egui::TextEdit::multiline(&mut self.arguments)
                .desired_width(width)
                .desired_rows(4)
                .code_editor(),
        );
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(egui::Button::new("Save profile").fill(ui.visuals().selection.bg_fill))
                .clicked()
            {
                self.notice = self.save(path).map_or_else(
                    |error| format!("Could not save: {error:#}"),
                    |()| "Profile saved".into(),
                );
            }
            if let Some(index) = self.selected {
                crate::icons::menu(ui, "Shell profile actions", |ui| {
                    if ui.button("Remove profile").clicked() {
                        let mut profiles = self.profiles.clone();
                        profiles.remove(index);
                        match self.persist(path, profiles) {
                            Ok(()) => {
                                self.begin(None);
                                self.notice = "Profile removed".into();
                            }
                            Err(error) => {
                                self.notice = format!("Could not remove profile: {error:#}")
                            }
                        }
                        ui.close_menu();
                    }
                });
            }
        });
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        choose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_roundtrip_keeps_argument_boundaries() -> Result<()> {
        let dir = tempfile::tempdir_in(std::env::current_dir()?)?;
        let path = dir.path().join("shells.json");
        let mut shells = Shells::default();
        shells.draft.name = "Custom test".into();
        shells.draft.executable = std::env::current_exe()?.display().to_string();
        shells.arguments = "--directory\nfolder with spaces\n--quiet".into();
        shells.save(&path)?;
        let reloaded = Shells::load(&path);
        assert_eq!(
            reloaded.profiles[0].args,
            ["--directory", "folder with spaces", "--quiet"]
        );
        assert_eq!(reloaded.profiles[0].name, "Custom test");
        Ok(())
    }
}
