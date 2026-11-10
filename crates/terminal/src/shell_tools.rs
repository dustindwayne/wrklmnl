use std::path::{Path, PathBuf};
pub fn conda() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CONDA_EXE")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(p);
    }
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)?;
    for base in ["anaconda3", "miniconda3", "miniforge3", "mambaforge"] {
        let root = home.join(base);
        let exe = root.join(if cfg!(windows) {
            "Scripts/conda.exe"
        } else {
            "bin/conda"
        });
        if exe.is_file() {
            return Some(exe);
        }
    }
    find("conda")
}
pub fn find(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        for ext in if cfg!(windows) {
            vec!["", ".exe", ".cmd", ".bat"]
        } else {
            vec![""]
        } {
            let p = dir.join(format!("{name}{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}
pub fn path(environment: Option<&Path>) -> std::ffi::OsString {
    let mut dirs = vec![];
    if let Some(root) = environment {
        dirs.push(root.to_path_buf());
        dirs.push(root.join("Scripts"));
        dirs.push(root.join("bin"));
        dirs.push(root.join("Library/bin"));
    }
    if let Some(exe) = conda() {
        if let Some(root) = exe.parent().and_then(Path::parent) {
            dirs.push(root.join("condabin"));
        }
    }
    if cfg!(windows) {
        if let Some(base) = std::env::var_os("ProgramFiles") {
            let dir = PathBuf::from(base).join("GitHub CLI");
            if dir.is_dir() {
                dirs.push(dir);
            }
        }
    }
    dirs.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(dirs).unwrap_or_default()
}
fn quote_ps(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}
pub fn arguments(shell: &str, args: &[String]) -> Vec<String> {
    let name = Path::new(shell)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    #[cfg(target_os = "linux")]
    if name == "bash" && args.is_empty() {
        if let Ok(init) = bash_init(false) {
            return vec![
                "--rcfile".into(),
                init.to_string_lossy().into_owned(),
                "-i".into(),
            ];
        }
    }
    if name == "pwsh" || name == "powershell" {
        if args.iter().any(|a| {
            ["-command", "-c", "-file", "-f", "-encodedcommand"]
                .contains(&a.to_lowercase().as_str())
        }) {
            return args.to_vec();
        }
        let mut out = args.to_vec();
        if !out.iter().any(|a| a.eq_ignore_ascii_case("-NoLogo")) {
            out.push("-NoLogo".into());
        }
        if !out.iter().any(|a| a.eq_ignore_ascii_case("-NoExit")) {
            out.push("-NoExit".into());
        }
        let mut script = String::new();
        if let Some(root) = conda().and_then(|exe| exe.parent()?.parent().map(Path::to_path_buf)) {
            let hook = root.join("shell/condabin/conda-hook.ps1");
            if hook.is_file() {
                script.push_str(&format!(". {}; ", quote_ps(&hook)));
            }
        }
        // Prompt metadata carries cwd/environment only. It does not execute shell text received from a UI document.
        script.push_str("$global:__TerminalPrompt=${function:prompt}; function global:prompt { $p=$ExecutionContext.SessionState.Path.CurrentFileSystemLocation.Path; $e=[string]$env:CONDA_PREFIX; [Console]::Write(([char]27)+']777;context;'+[Uri]::EscapeDataString($p)+';'+[Uri]::EscapeDataString($e)+([char]7)); & $global:__TerminalPrompt }; ");
        script.push_str("if (Get-Module -ListAvailable PSReadLine) { Import-Module PSReadLine -ErrorAction SilentlyContinue }; ");
        script.push_str("if (Get-Command gh -ErrorAction SilentlyContinue) { gh completion -s powershell | Out-String | Invoke-Expression }; ");
        out.extend(["-Command".into(), script]);
        return out;
    }
    args.to_vec()
}

#[cfg(windows)]
pub fn cmd_conda_hook(args: &[String]) -> Option<PathBuf> {
    if args.iter().any(|arg| {
        arg.get(..2).is_some_and(|prefix| {
            prefix.eq_ignore_ascii_case("/c") || prefix.eq_ignore_ascii_case("/k")
        })
    }) {
        return None;
    }
    let executable = conda()?;
    let hook = executable
        .parent()?
        .parent()?
        .join("condabin")
        .join("conda_hook.bat");
    hook.is_file().then_some(hook)
}

/// Private Bash startup file: preserve user configuration, then add context
/// metadata without changing the visible prompt or global shell files.
#[cfg(target_os = "linux")]
pub fn bash_init(activate_base: bool) -> anyhow::Result<PathBuf> {
    let state = crate::history::default_path();
    let folder = state
        .parent()
        .ok_or_else(|| anyhow::anyhow!("No state directory"))?;
    let path = folder.join(if activate_base {
        "bash-conda-init.sh"
    } else {
        "bash-init.sh"
    });
    let mut script = String::from("[ ! -f \"$HOME/.bashrc\" ] || . \"$HOME/.bashrc\"\n");
    if let Some(root) =
        conda().and_then(|p| p.parent().and_then(Path::parent).map(Path::to_path_buf))
    {
        let hook = root.join("etc/profile.d/conda.sh");
        if hook.is_file() {
            script.push_str(&format!(
                ". '{}'\n",
                hook.to_string_lossy().replace('\'', "'\\''")
            ));
            if activate_base {
                script.push_str("conda activate base\n");
            }
        }
    }
    script.push_str(r#"__dterm_context() {
    local p="$PWD" e="${CONDA_PREFIX-}"
    p=${p//%/%25}; p=${p//;/%3B}; p=${p//$'\e'/%1B}; p=${p//$'\a'/%07}; p=${p//$'\n'/%0A}; p=${p//$'\r'/%0D}
    e=${e//%/%25}; e=${e//;/%3B}; e=${e//$'\e'/%1B}; e=${e//$'\a'/%07}; e=${e//$'\n'/%0A}; e=${e//$'\r'/%0D}
    printf '\e]777;context;%s;%s\a' "$p" "$e"
}
if declare -p PROMPT_COMMAND 2>/dev/null | grep -q 'declare -a'; then
    PROMPT_COMMAND+=(__dterm_context)
else
    PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND; }__dterm_context"
fi
"#);
    crate::persistence::atomic_write(&path, script.as_bytes())?;
    Ok(path)
}
pub fn decode(text: &[u8]) -> String {
    let mut bytes = vec![];
    let mut i = 0;
    while i < text.len() {
        if text[i] == b'%' && i + 2 < text.len() {
            if let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&text[i + 1..i + 3]).unwrap_or(""), 16)
            {
                bytes.push(v);
                i += 3;
                continue;
            }
        }
        bytes.push(text[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_shell_programs_are_not_wrapped() {
        let args = vec!["-NoProfile".into(), "-Command".into(), "echo test".into()];
        assert_eq!(arguments("pwsh.exe", &args), args);
    }
    #[test]
    fn context_paths_decode_reserved_characters() {
        assert_eq!(decode(b"C%3A%5Cproject%20name"), r"C:\project name");
    }
}
