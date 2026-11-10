use crate::completion::Candidate;
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
fn item(text: &str, detail: &str, start: usize, end: usize) -> Candidate {
    Candidate {
        label: text.into(),
        insert: text.into(),
        detail: detail.into(),
        start,
        end,
    }
}
pub fn complete(
    line: &str,
    cursor: usize,
    cwd: &Path,
    environment: Option<&Path>,
) -> Option<(Vec<Candidate>, String)> {
    let before = &line[..cursor];
    let start = before
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let word = &before[start..];
    let prior = before[..start].split_whitespace().collect::<Vec<_>>();
    let command = prior.first().copied().unwrap_or(word);
    if command == "gh" && !prior.is_empty() {
        if let Some(exe) = crate::shell_tools::find("gh") {
            let mut cmd = Command::new(exe);
            cmd.arg("__complete")
                .args(&prior[1..])
                .arg(word)
                .current_dir(cwd);
            if let Ok(bytes) = crate::completion::output_timeout(cmd, Duration::from_secs(3)) {
                let text = String::from_utf8_lossy(&bytes);
                let items = text
                    .lines()
                    .filter(|l| !l.starts_with(':'))
                    .filter_map(|l| {
                        let mut p = l.splitn(2, '\t');
                        let label = p.next()?;
                        if label.is_empty() {
                            return None;
                        }
                        Some(item(label, p.next().unwrap_or("GitHub CLI"), start, cursor))
                    })
                    .collect::<Vec<_>>();
                if !items.is_empty() {
                    return Some((items, "GitHub CLI / installed command completion".into()));
                }
            }
        }
    }
    if command == "conda" {
        if prior.len() <= 1 {
            let values = [
                "activate",
                "deactivate",
                "create",
                "env",
                "install",
                "list",
                "info",
                "remove",
                "run",
                "search",
                "update",
                "config",
                "clean",
            ];
            let items = values
                .iter()
                .filter(|s| s.starts_with(word))
                .map(|s| item(s, "Conda", start, cursor))
                .collect();
            return Some((items, "Conda commands".into()));
        }
        if matches!(prior.get(1), Some(&"activate"))
            || prior.windows(2).any(|p| p == ["env", "remove"])
        {
            if let Some(exe) = crate::shell_tools::conda() {
                let mut cmd = Command::new(exe);
                cmd.args(["env", "list", "--json"]);
                if let Ok(bytes) = crate::completion::output_timeout(cmd, Duration::from_secs(5)) {
                    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        let items = value["envs"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v.as_str())
                            .map(PathBuf::from)
                            .filter_map(|p| {
                                let name = if p.file_name().is_some_and(|n| {
                                    n == "anaconda3" || n == "miniconda3" || n == "miniforge3"
                                }) {
                                    "base".to_string()
                                } else {
                                    p.file_name()?.to_string_lossy().into_owned()
                                };
                                name.starts_with(word)
                                    .then(|| item(&name, &p.display().to_string(), start, cursor))
                            })
                            .collect();
                        return Some((items, "Conda / installed environments".into()));
                    }
                }
            }
        }
    }
    let mut root = cwd;
    loop {
        if let Ok(bytes) = std::fs::read(root.join("package.json")) {
            if let Ok(package) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let run = matches!(command, "npm" | "pnpm" | "yarn" | "bun")
                    && (prior.get(1) == Some(&"run") || command == "yarn" && prior.len() == 1);
                if run {
                    let items = package["scripts"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .filter(|(name, _)| name.starts_with(word))
                        .map(|(name, value)| {
                            item(
                                name,
                                value.as_str().unwrap_or("Project script"),
                                start,
                                cursor,
                            )
                        })
                        .collect();
                    return Some((items, format!("Project scripts / {}", root.display())));
                }
                let tool = if matches!(command, "npx" | "pnpm" | "bunx") {
                    prior.get(1).copied().unwrap_or("")
                } else {
                    command
                };
                if !tool.is_empty() {
                    let bin = root
                        .join("node_modules/.bin")
                        .join(format!("{tool}{}", if cfg!(windows) { ".cmd" } else { "" }));
                    if bin.is_file() && prior.len() <= 2 {
                        let mut cmd = if cfg!(windows) {
                            let mut c = Command::new("cmd.exe");
                            c.args(["/D", "/C"]).arg(&bin);
                            c
                        } else {
                            Command::new(&bin)
                        };
                        cmd.arg("--help").current_dir(root);
                        if let Ok(bytes) =
                            crate::completion::output_timeout(cmd, Duration::from_secs(3))
                        {
                            let text = String::from_utf8_lossy(&bytes);
                            let mut names = std::collections::BTreeSet::new();
                            for l in text.lines() {
                                let l = l.trim();
                                if let Some(first) = l.split_whitespace().next() {
                                    let first = first.trim_matches(',');
                                    if first.starts_with(word)
                                        && !first.is_empty()
                                        && first
                                            .chars()
                                            .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
                                    {
                                        names.insert(first.to_string());
                                    }
                                }
                            }
                            let items = names
                                .iter()
                                .map(|n| item(n, "Installed project CLI", start, cursor))
                                .collect::<Vec<_>>();
                            if !items.is_empty() {
                                return Some((items, format!("Installed project CLI / {tool}")));
                            }
                        }
                    }
                }
            }
        }
        if let Some(parent) = root.parent() {
            root = parent;
        } else {
            break;
        }
    }
    if command == "python" && prior.get(1) == Some(&"-m") {
        let exe = environment
            .map(|root| {
                root.join(if cfg!(windows) {
                    "python.exe"
                } else {
                    "bin/python"
                })
            })
            .filter(|p| p.is_file())
            .or_else(|| crate::shell_tools::find("python"));
        if let Some(exe) = exe {
            let mut cmd = Command::new(exe);
            cmd.args(["-c","import pkgutil,json; print(json.dumps(sorted({m.name for m in pkgutil.iter_modules()})))"]).current_dir(cwd);
            if let Ok(bytes) = crate::completion::output_timeout(cmd, Duration::from_secs(3)) {
                if let Ok(modules) = serde_json::from_slice::<Vec<String>>(&bytes) {
                    let items = modules
                        .iter()
                        .filter(|s| s.starts_with(word))
                        .map(|s| item(s, "Installed Python module", start, cursor))
                        .collect();
                    return Some((items, "Active Python environment / modules".into()));
                }
            }
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conda_commands_complete_without_execution() {
        let (items, _) = complete("conda act", 9, Path::new("."), None).unwrap();
        assert!(items.iter().any(|c| c.insert == "activate"));
    }
    #[test]
    fn completion_uses_project_scripts() {
        let dir = std::env::temp_dir().join(format!("terminal-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            br#"{"scripts":{"dev":"vite","test":"vitest"}}"#,
        )
        .unwrap();
        let (items, _) = complete("npm run de", 10, &dir, None).unwrap();
        assert_eq!(items[0].insert, "dev");
        std::fs::remove_file(dir.join("package.json")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
