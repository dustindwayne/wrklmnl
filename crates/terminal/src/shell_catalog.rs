//! Discover launchable shells/interactive CLIs without blocking the UI thread.
use eframe::egui;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};
#[derive(Clone, Debug, serde::Serialize)]
pub struct Choice {
    pub label: String,
    pub detail: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
}
impl Choice {
    pub fn icon_path(&self) -> PathBuf {
        if self.label == "Git Bash" {
            if let Some(root) = self.executable.parent().and_then(Path::parent) {
                let branded = root.join("git-bash.exe");
                if branded.is_file() {
                    return branded;
                }
            }
        }
        if self.label.contains("Conda") && !self.label.contains("Python") {
            if let Some(conda) = crate::shell_tools::conda() {
                if let Some(root) = conda.parent().and_then(Path::parent) {
                    for name in [
                        "Menu/anaconda-navigator.ico",
                        "Menu/anaconda.ico",
                        "Scripts/conda.exe",
                    ] {
                        let candidate = root.join(name);
                        if candidate.is_file() {
                            return candidate;
                        }
                    }
                }
                return conda;
            }
        }
        self.executable.clone()
    }
}

pub fn executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return path
            .metadata()
            .is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
    }
    #[cfg(not(unix))]
    {
        !path.to_string_lossy().contains("WindowsApps")
            || path.metadata().is_ok_and(|m| m.len() > 0)
    }
}
fn add(
    items: &mut Vec<Choice>,
    label: &str,
    detail: &str,
    candidates: impl IntoIterator<Item = PathBuf>,
    args: Vec<String>,
) {
    if let Some(path) = candidates.into_iter().find(|p| executable(p)) {
        // Unix launchers such as /snap/bin/pwsh depend on argv[0]. Resolving
        // their symlink to /usr/bin/snap would launch the wrong program.
        let path = if cfg!(windows) {
            path.canonicalize().unwrap_or(path)
        } else {
            path
        };
        if !items.iter().any(|c| {
            c.executable.canonicalize().ok() == path.canonicalize().ok()
                && c.executable.file_name() == path.file_name()
                && c.args == args
        }) {
            items.push(Choice {
                label: label.into(),
                detail: detail.into(),
                executable: path,
                args,
            });
        }
    }
}
fn find(name: &str) -> Vec<PathBuf> {
    crate::shell_tools::find(name).into_iter().collect()
}
fn wsl_names(bytes: &[u8]) -> Vec<String> {
    let text = if bytes.iter().any(|&b| b == 0) {
        let units: Vec<_> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("docker-desktop"))
        .map(str::to_owned)
        .collect()
}
pub fn discover() -> Vec<Choice> {
    let mut items = vec![];
    if cfg!(windows) {
        let programs = std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|| "C:/Program Files".into());
        let windows = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| "C:/Windows".into());
        let mut paths = find("pwsh");
        paths.push(programs.join("PowerShell/7/pwsh.exe"));
        add(
            &mut items,
            "PowerShell 7",
            "Modern PowerShell",
            paths,
            vec![],
        );
        let mut paths = find("powershell");
        paths.push(windows.join("System32/WindowsPowerShell/v1.0/powershell.exe"));
        add(
            &mut items,
            "Windows PowerShell",
            "Windows scripting shell",
            paths,
            vec![],
        );
        let mut paths = find("cmd");
        paths.push(windows.join("System32/cmd.exe"));
        add(
            &mut items,
            "Command Prompt",
            "Windows command shell",
            paths,
            vec![],
        );
        let mut paths = vec![programs.join("Git/bin/bash.exe")];
        if let Some(base) = std::env::var_os("LOCALAPPDATA") {
            paths.push(PathBuf::from(base).join("Programs/Git/bin/bash.exe"));
        }
        add(
            &mut items,
            "Git Bash",
            "Git's Bash environment",
            paths,
            vec!["--login".into(), "-i".into()],
        );
        if let Some(wsl) = crate::shell_tools::find("wsl").or_else(|| {
            let p = windows.join("System32/wsl.exe");
            p.is_file().then_some(p)
        }) {
            let mut command = std::process::Command::new(&wsl);
            command.args(["--list", "--quiet"]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            if let Ok(bytes) = crate::completion::output_timeout(command, Duration::from_secs(3)) {
                for name in wsl_names(&bytes) {
                    add(
                        &mut items,
                        &name,
                        "WSL Linux shell",
                        [wsl.clone()],
                        vec!["--distribution".into(), name.clone()],
                    );
                }
            }
        }
    } else {
        let shells = std::fs::read_to_string("/etc/shells").unwrap_or_default();
        for (name, label) in [
            ("bash", "Bash"),
            ("zsh", "Zsh"),
            ("fish", "Fish"),
            ("nu", "Nushell"),
            ("pwsh", "PowerShell"),
            ("sh", "POSIX shell"),
        ] {
            let mut paths = find(name);
            paths.extend(
                shells
                    .lines()
                    .filter(|s| !s.starts_with('#'))
                    .map(PathBuf::from)
                    .filter(|p| p.file_name().is_some_and(|n| n == name)),
            );
            add(&mut items, label, "Interactive shell", paths, vec![]);
        }
        if let Some(path) = std::env::var_os("SHELL") {
            let p = PathBuf::from(path);
            let label = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            add(&mut items, &label, "Login shell", [p], vec![]);
        }
    }
    add(
        &mut items,
        "Nushell",
        "Structured-data shell",
        find("nu"),
        vec![],
    );
    if let Some(conda) = crate::shell_tools::conda() {
        #[cfg(target_os = "linux")]
        if let (Some(bash), Ok(init)) = (
            crate::shell_tools::find("bash"),
            crate::shell_tools::bash_init(true),
        ) {
            add(
                &mut items,
                "Conda",
                "Bash · base environment",
                [bash],
                vec![
                    "--rcfile".into(),
                    init.to_string_lossy().into_owned(),
                    "-i".into(),
                ],
            );
        }
        if let Some(shell) = items
            .iter()
            .find(|c| c.label == "PowerShell 7" || c.label == "Windows PowerShell")
            .cloned()
        {
            let mut args = crate::shell_tools::arguments(
                &shell.executable.to_string_lossy(),
                &["-NoProfile".into()],
            );
            if let Some(script) = args.last_mut() {
                script.push_str("conda activate base");
            }
            add(
                &mut items,
                "Anaconda / Conda",
                "PowerShell · base environment",
                [shell.executable],
                args,
            );
        }
        if let Some(root) = conda.parent().and_then(Path::parent) {
            add(
                &mut items,
                "Conda Python",
                "Anaconda Python REPL",
                [root.join(if cfg!(windows) {
                    "python.exe"
                } else {
                    "bin/python"
                })],
                vec!["-i".into()],
            );
        }
    }
    add(
        &mut items,
        "Python",
        "Interactive Python",
        find(if cfg!(windows) { "python" } else { "python3" }),
        vec!["-i".into()],
    );
    add(
        &mut items,
        "Node.js",
        "Interactive JavaScript",
        find("node"),
        vec!["-i".into()],
    );
    items
}
#[derive(Default)]
pub struct Catalog {
    items: Vec<Choice>,
    receiver: Option<mpsc::Receiver<Vec<Choice>>>,
    started: bool,
}
impl Catalog {
    fn scan(&mut self) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(discover());
        });
        self.receiver = Some(rx);
        self.started = true;
    }
    pub fn refresh(&mut self) {
        if self.receiver.is_none() {
            self.scan();
        }
    }
    pub fn items(&self) -> &[Choice] {
        &self.items
    }
    pub fn loading(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        if !self.started {
            self.scan();
        }
        if let Some(rx) = &self.receiver {
            match rx.try_recv() {
                Ok(items) => {
                    self.items = items;
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.receiver = None,
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100))
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wsl_output_handles_both_encodings() {
        assert_eq!(wsl_names(b"Ubuntu\r\nDebian\r\n"), ["Ubuntu", "Debian"]);
        let bytes: Vec<_> = "\u{feff}Ubuntu\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(wsl_names(&bytes), ["Ubuntu"]);
    }
    #[test]
    fn nonexistent_candidates_are_omitted() {
        let mut items = vec![];
        add(
            &mut items,
            "Missing",
            "",
            [PathBuf::from("/not/a/real/shell")],
            vec![],
        );
        assert!(items.is_empty());
    }
}
