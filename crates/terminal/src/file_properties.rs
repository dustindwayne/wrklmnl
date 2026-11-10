use eframe::egui;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub struct Properties {
    path: PathBuf,
    name: String,
    modified: String,
    accessed: String,
    section: u8,
    error: String,
}

impl Properties {
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let metadata = fs::symlink_metadata(&path).ok();
        let modified = metadata
            .as_ref()
            .and_then(|value| value.modified().ok())
            .map(timestamp)
            .unwrap_or_default();
        let accessed = metadata
            .as_ref()
            .and_then(|value| value.accessed().ok())
            .map(timestamp)
            .unwrap_or_default();
        Self {
            path,
            name,
            modified,
            accessed,
            section: 0,
            error: String::new(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> (bool, bool, Option<PathBuf>) {
        let mut open = true;
        let mut changed = false;
        let mut launch = None;
        egui::Window::new("File properties")
            .open(&mut open)
            .default_width(420.0)
            .default_pos(ctx.screen_rect().right_bottom() - egui::vec2(450.0, 330.0))
            .show(ctx, |ui| {
                ui.heading(
                    self.path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .as_ref(),
                );
                ui.horizontal_wrapped(|ui| {
                    if crate::widgets::button(ui, "Open").clicked() {
                        launch = Some(self.path.clone());
                    }
                    if crate::widgets::button(ui, "Copy path").clicked() {
                        ui.ctx().copy_text(self.path.display().to_string());
                    }
                });
                ui.add(
                    egui::Label::new(self.path.display().to_string())
                        .wrap()
                        .selectable(true),
                );
                ui.horizontal(|ui| {
                    for (section, label) in [(0, "General"), (1, "Timestamps"), (2, "Permissions")]
                    {
                        ui.selectable_value(&mut self.section, section, label);
                    }
                });
                ui.separator();
                let metadata = match fs::symlink_metadata(&self.path) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        ui.label(error.to_string());
                        return;
                    }
                };
                let link = metadata.is_symlink() || is_reparse_point(&metadata);
                match self.section {
                    0 => {
                        let (_, kind) = crate::launcher::file_kind(&self.path, metadata.is_dir());
                        ui.label(if link { "Link" } else { kind });
                        if metadata.is_file() {
                            ui.label(format!("Size · {} bytes", metadata.len()));
                        }
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            let edit = ui.text_edit_singleline(&mut self.name);
                            let enter =
                                edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if ui.button("Rename").clicked() || enter {
                                let result = validate_name(&self.name).and_then(|()| {
                                    let destination = self.path.with_file_name(&self.name);
                                    if destination != self.path {
                                        move_no_replace(&self.path, &destination)?;
                                        self.path = destination;
                                    }
                                    Ok(())
                                });
                                changed |= self.report(result);
                            }
                        });
                    }
                    1 => {
                        if let Ok(time) = metadata.created() {
                            ui.label(format!("Created · {} UTC", timestamp(time)));
                        }
                        if link || !metadata.is_file() {
                            if let Ok(time) = metadata.modified() {
                                ui.label(format!("Modified · {} UTC", timestamp(time)));
                            }
                            if let Ok(time) = metadata.accessed() {
                                ui.label(format!("Accessed · {} UTC", timestamp(time)));
                            }
                        } else {
                            ui.weak("UTC · YYYY-MM-DD HH:MM:SS. Leave a field blank to skip it.");
                            egui::Grid::new("file_times").show(ui, |ui| {
                                ui.label("Modified");
                                ui.text_edit_singleline(&mut self.modified);
                                ui.end_row();
                                ui.label("Accessed");
                                ui.text_edit_singleline(&mut self.accessed);
                                ui.end_row();
                            });
                            if ui.button("Apply timestamps").clicked() {
                                let result =
                                    apply_times(&self.path, &self.modified, &self.accessed);
                                changed |= self.report(result);
                            }
                        }
                    }
                    _ => {
                        if !link {
                            changed |= self.permissions(ui, &metadata);
                        } else {
                            ui.weak("Permission edits are available on the target file.");
                        }
                        #[cfg(windows)]
                        if ui.button("Windows properties and security").clicked() {
                            let result = windows_properties(&self.path);
                            self.report(result);
                        }
                    }
                }
                if !self.error.is_empty() {
                    ui.colored_label(ui.visuals().error_fg_color, &self.error);
                }
            });
        (open, changed, launch)
    }

    fn report(&mut self, result: anyhow::Result<()>) -> bool {
        match result {
            Ok(()) => {
                self.error.clear();
                true
            }
            Err(error) => {
                self.error = format!("{error:#}");
                false
            }
        }
    }

    fn permissions(&mut self, ui: &mut egui::Ui, metadata: &fs::Metadata) -> bool {
        let mut permissions = metadata.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut mode = permissions.mode();
            let before = mode;
            egui::Grid::new("file_permissions").show(ui, |ui| {
                for (label, shift) in [("Owner", 6), ("Group", 3), ("Others", 0)] {
                    ui.label(label);
                    for (label, bit) in [("Read", 4), ("Write", 2), ("Execute", 1)] {
                        let mask = bit << shift;
                        let mut enabled = mode & mask != 0;
                        if ui.checkbox(&mut enabled, label).changed() {
                            if enabled {
                                mode |= mask;
                            } else {
                                mode &= !mask;
                            }
                        }
                    }
                    ui.end_row();
                }
            });
            if mode == before {
                return false;
            }
            permissions.set_mode(mode);
        }
        #[cfg(not(unix))]
        {
            let mut readonly = permissions.readonly();
            if !ui.checkbox(&mut readonly, "Read-only").changed() {
                return false;
            }
            permissions.set_readonly(readonly);
        }
        self.report(fs::set_permissions(&self.path, permissions).map_err(Into::into))
    }
}

fn apply_times(path: &Path, modified: &str, accessed: &str) -> anyhow::Result<()> {
    let parse = |text: &str| -> anyhow::Result<Option<std::time::SystemTime>> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(
            chrono::NaiveDateTime::parse_from_str(text.trim(), "%Y-%m-%d %H:%M:%S")?
                .and_utc()
                .into(),
        ))
    };
    let mut times = fs::FileTimes::new();
    if let Some(time) = parse(modified)? {
        times = times.set_modified(time);
    }
    if let Some(time) = parse(accessed)? {
        times = times.set_accessed(time);
    }
    fs::File::options()
        .write(true)
        .open(path)?
        .set_times(times)?;
    Ok(())
}

fn timestamp(value: std::time::SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(value)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

pub fn import_file(source: &Path, folder: &Path, root: &Path) -> anyhow::Result<()> {
    let target = folder.canonicalize()?;
    anyhow::ensure!(
        target.starts_with(root.canonicalize()?),
        "Destination is outside the workspace"
    );
    let metadata = fs::symlink_metadata(source)?;
    anyhow::ensure!(
        metadata.is_file() && !is_reparse_point(&metadata),
        "Only regular files can be imported"
    );
    let name = source
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Missing file name"))?;
    let mut staged = tempfile::NamedTempFile::new_in(&target)?;
    std::io::copy(&mut fs::File::open(source)?, staged.as_file_mut())?;
    staged.as_file().sync_all()?;
    staged.persist_noclobber(target.join(name))?;
    Ok(())
}

pub fn validate_name(name: &str) -> anyhow::Result<()> {
    let mut parts = Path::new(name).components();
    anyhow::ensure!(
        matches!(parts.next(), Some(Component::Normal(value)) if value == name)
            && parts.next().is_none(),
        "Enter a single file or folder name"
    );
    #[cfg(windows)]
    anyhow::ensure!(
        !name.contains([':', '<', '>', '"', '|', '?', '*']) && !name.ends_with([' ', '.']),
        "Invalid Windows file name"
    );
    Ok(())
}

pub fn move_into(source: &Path, folder: &Path, root: &Path) -> anyhow::Result<()> {
    let root = root.canonicalize()?;
    let parent = source
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing source folder"))?;
    anyhow::ensure!(
        parent.canonicalize()?.starts_with(&root),
        "Source is outside the workspace"
    );
    let target = folder.canonicalize()?;
    anyhow::ensure!(
        target.starts_with(&root),
        "Destination is outside the workspace"
    );
    let metadata = fs::symlink_metadata(source)?;
    if metadata.is_dir() && !is_reparse_point(&metadata) {
        anyhow::ensure!(
            !target.starts_with(source.canonicalize()?),
            "A folder cannot be moved into itself"
        );
    }
    let name = source
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Missing file name"))?;
    move_no_replace(source, &target.join(name))
}

fn move_no_replace(from: &Path, to: &Path) -> anyhow::Result<()> {
    #[cfg(windows)]
    let moved = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileW(wide(from).as_ptr(), wide(to).as_ptr())
            != 0
    };
    #[cfg(unix)]
    let moved = {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(from.as_os_str().as_bytes())?;
        let to = std::ffi::CString::new(to.as_os_str().as_bytes())?;
        #[cfg(target_os = "linux")]
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let result = return Err(anyhow::anyhow!(
            "Moving files is unavailable on this platform"
        ));
        result == 0
    };
    if !moved {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

pub fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

#[cfg(windows)]
fn wide(value: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
fn windows_properties(path: &Path) -> anyhow::Result<()> {
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW,
    };
    let path = wide(path);
    let verb = wide(Path::new("properties"));
    let mut args: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    args.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    args.fMask = SEE_MASK_INVOKEIDLIST;
    args.lpVerb = verb.as_ptr();
    args.lpFile = path.as_ptr();
    args.nShow = 1;
    if unsafe { ShellExecuteExW(&mut args) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_and_imports_never_replace_existing_files() -> anyhow::Result<()> {
        let root = tempfile::tempdir_in(std::env::current_dir()?)?;
        let folder = root.path().join("destination");
        fs::create_dir(&folder)?;
        let source = root.path().join("test.txt");
        let existing = folder.join("test.txt");
        fs::write(&source, "source")?;
        fs::write(&existing, "existing")?;
        assert!(move_into(&source, &folder, root.path()).is_err());
        assert!(import_file(&source, &folder, root.path()).is_err());
        assert_eq!(fs::read_to_string(&existing)?, "existing");
        assert_eq!(fs::read_to_string(&source)?, "source");
        fs::remove_file(&existing)?;
        import_file(&source, &folder, root.path())?;
        assert_eq!(fs::read_to_string(&existing)?, "source");
        fs::remove_file(&existing)?;
        move_into(&source, &folder, root.path())?;
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&existing)?, "source");
        Ok(())
    }
}
