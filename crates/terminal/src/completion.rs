use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub enum Context {
    Commands {
        shell: String,
        cwd: PathBuf,
        environment: Option<PathBuf>,
    },
    Code {
        language: String,
        cwd: PathBuf,
        file: Option<PathBuf>,
    },
}
#[derive(Clone, Debug)]
pub struct Candidate {
    pub label: String,
    pub insert: String,
    pub detail: String,
    pub start: usize,
    pub end: usize,
}
struct Request {
    id: u64,
    text: String,
    cursor: usize,
    context: Context,
}
pub struct Answer {
    id: u64,
    pub items: Vec<Candidate>,
    pub provider: String,
}
pub struct Completion {
    tx: SyncSender<Request>,
    rx: Receiver<Answer>,
    generation: u64,
    pub items: Vec<Candidate>,
    pub provider: String,
    pub selected: usize,
    last: String,
    pub applied_cursor: Option<usize>,
    wake: std::sync::Arc<std::sync::Mutex<Option<eframe::egui::Context>>>,
}
impl Default for Completion {
    fn default() -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(2);
        let (results, rx) = mpsc::channel();
        let wake = std::sync::Arc::new(std::sync::Mutex::new(None::<eframe::egui::Context>));
        let notify = wake.clone();
        thread::spawn(move || {
            let mut lsp: Option<(String, Lsp)> = None;
            for mut request in &requests {
                while let Ok(new) = requests.recv_timeout(Duration::from_millis(180)) {
                    request = new;
                }
                let answer = complete(request, &mut lsp);
                let sent = results.send(answer);
                if let Some(ctx) = notify.lock().unwrap().as_ref() {
                    ctx.request_repaint();
                }
                if sent.is_err() {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            generation: 0,
            items: vec![],
            provider: String::new(),
            selected: 0,
            last: String::new(),
            applied_cursor: None,
            wake,
        }
    }
}
impl Completion {
    pub fn set_context(&mut self, ctx: &eframe::egui::Context) {
        *self.wake.lock().unwrap() = Some(ctx.clone());
    }
    pub fn dismiss(&mut self) {
        self.generation += 1;
        self.items.clear();
        self.last.clear();
    }
    pub fn keyboard(&mut self, ui: &mut eframe::egui::Ui, focus: bool) {
        if !focus || self.items.is_empty() {
            return;
        }
        use eframe::egui::{Key, Modifiers};
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.dismiss();
            return;
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            self.selected = (self.selected + 1) % self.items.len();
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            self.selected = (self.selected + self.items.len() - 1) % self.items.len();
        }
    }
    pub fn restore_cursor(&mut self, ui: &mut eframe::egui::Ui, text: &str, id: eframe::egui::Id) {
        if let Some(byte) = self.applied_cursor.take() {
            let pos = text[..byte.min(text.len())].chars().count();
            let mut state =
                eframe::egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(eframe::egui::text::CCursorRange::one(
                    eframe::egui::text::CCursor::new(pos),
                )));
            state.store(ui.ctx(), id);
            ui.memory_mut(|m| m.request_focus(id));
        }
    }
    pub fn popup(
        &mut self,
        ui: &mut eframe::egui::Ui,
        text: &mut String,
        anchor: eframe::egui::Rect,
        id: eframe::egui::Id,
    ) {
        *self.wake.lock().unwrap() = Some(ui.ctx().clone());
        if self.items.is_empty() {
            return;
        }
        let height = (self.items.len().min(6) as f32 * 28.0 + 36.0).min(210.0);
        let pos = eframe::egui::pos2(anchor.left(), (anchor.top() - height - 8.0).max(8.0));
        eframe::egui::Area::new(id.with("completion_popup"))
            .order(eframe::egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                eframe::egui::Frame::NONE
                    .inner_margin(12.0)
                    .fill(ui.visuals().window_fill)
                    .show(ui, |ui| {
                        ui.set_max_width(640.0);
                        self.choices(ui, text);
                    });
            });
        self.restore_cursor(ui, text, id);
    }
    pub fn request(&mut self, text: &str, cursor: usize, context: Context, force: bool) {
        let key = format!("{text}:{cursor}");
        if !force && key == self.last {
            return;
        }
        let generation = self.generation + 1;
        let r = Request {
            id: generation,
            text: text.into(),
            cursor: cursor.min(text.len()),
            context,
        };
        if self.tx.try_send(r).is_ok() {
            self.generation = generation;
            self.last = key;
            self.items.clear();
            self.selected = 0;
        }
    }
    pub fn poll(&mut self) {
        while let Ok(answer) = self.rx.try_recv() {
            if answer.id == self.generation {
                self.items = answer.items;
                self.provider = answer.provider;
                self.selected = 0;
            }
        }
    }
    pub fn apply(&mut self, text: &mut String) -> bool {
        let Some(item) = self.items.get(self.selected) else {
            return false;
        };
        if item.start <= item.end
            && text.is_char_boundary(item.start)
            && text.is_char_boundary(item.end)
            && item.end <= text.len()
        {
            text.replace_range(item.start..item.end, &item.insert);
            self.applied_cursor = Some(item.start + item.insert.len());
            self.items.clear();
            self.last.clear();
            return true;
        }
        false
    }
    pub fn choices(&mut self, ui: &mut eframe::egui::Ui, text: &mut String) {
        self.poll();
        if self.items.is_empty() {
            return;
        }
        ui.label(eframe::egui::RichText::new(&self.provider).small());
        let mut chosen = None;
        eframe::egui::ScrollArea::vertical()
            .id_salt("completion_choices")
            .max_height(160.0)
            .show(ui, |ui| {
                for (i, item) in self.items.iter().take(30).enumerate() {
                    let label = format!("{}   {}", item.label, item.detail);
                    if ui
                        .add(
                            eframe::egui::Button::new(eframe::egui::RichText::new(
                                if i == self.selected {
                                    format!("> {label}")
                                } else {
                                    label
                                },
                            ))
                            .frame(false),
                        )
                        .clicked()
                    {
                        chosen = Some(i);
                    }
                }
            });
        if let Some(i) = chosen {
            self.selected = i;
            self.apply(text);
        }
    }
}
pub fn byte_cursor(text: &str, characters: usize) -> usize {
    text.char_indices()
        .nth(characters)
        .map(|x| x.0)
        .unwrap_or(text.len())
}
fn prefix(text: &str, cursor: usize) -> (usize, String) {
    let end = cursor.min(text.len());
    let start = text[..end]
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_alphanumeric() || "_$-./\\:%".contains(*c)))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    (start, text[start..end].to_string())
}
fn candidate(label: String, insert: String, detail: &str, start: usize, end: usize) -> Candidate {
    Candidate {
        label,
        insert,
        detail: detail.into(),
        start,
        end,
    }
}
fn complete(request: Request, lsp: &mut Option<(String, Lsp)>) -> Answer {
    let (start, word) = prefix(&request.text, request.cursor);
    let mut provider = String::new();
    let mut items = vec![];
    match &request.context {
        Context::Commands {
            shell,
            cwd,
            environment,
        } => {
            if let Some((items, provider)) = crate::project_completion::complete(
                &request.text,
                request.cursor,
                cwd,
                environment.as_deref(),
            ) {
                return Answer {
                    id: request.id,
                    items,
                    provider,
                };
            }
            if shell.to_lowercase().contains("pwsh") || shell.to_lowercase().contains("powershell")
            {
                let script="$ErrorActionPreference='Stop'; $r=[System.Management.Automation.CommandCompletion]::CompleteInput($env:TERMINAL_COMPLETE_LINE,[int]$env:TERMINAL_COMPLETE_CURSOR,$null); @{start=$r.ReplacementIndex;length=$r.ReplacementLength;items=@($r.CompletionMatches|Select-Object -First 80 CompletionText,ListItemText,ToolTip)}|ConvertTo-Json -Depth 5 -Compress";
                let mut command = Command::new(shell);
                command
                    .args([
                        "-NoLogo",
                        "-NoProfile",
                        "-NonInteractive",
                        "-Command",
                        script,
                    ])
                    .current_dir(cwd)
                    .env("PATH", crate::shell_tools::path(environment.as_deref()))
                    .env("TERMINAL_COMPLETE_LINE", &request.text)
                    .env(
                        "TERMINAL_COMPLETE_CURSOR",
                        request.text[..request.cursor]
                            .encode_utf16()
                            .count()
                            .to_string(),
                    );
                if let Ok(bytes) = output_timeout(command, Duration::from_secs(4)) {
                    if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
                        let a =
                            utf16_offset(&request.text, v["start"].as_u64().unwrap_or(0) as usize);
                        let b = utf16_offset(
                            &request.text,
                            (v["start"].as_u64().unwrap_or(0) + v["length"].as_u64().unwrap_or(0))
                                as usize,
                        );
                        if let Some(matches) = v["items"].as_array() {
                            for m in matches {
                                if let Some(insert) = m["CompletionText"].as_str() {
                                    items.push(candidate(
                                        m["ListItemText"].as_str().unwrap_or(insert).into(),
                                        insert.into(),
                                        m["ToolTip"].as_str().unwrap_or("PowerShell"),
                                        a,
                                        b,
                                    ));
                                }
                            }
                        }
                        provider =
                            "PowerShell native completion / isolated NoProfile provider".into();
                    }
                }
            }
            if items.is_empty() {
                items = command_candidates(&request.text, &word, start, request.cursor, shell, cwd);
                provider = format!(
                    "{} / OS executable paths, shell built-ins and filesystem",
                    Path::new(shell)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                );
            }
        }
        Context::Code {
            language,
            cwd,
            file,
        } => {
            let key = format!("{language}:{}:{file:?}", cwd.display());
            if !lsp.as_ref().is_some_and(|(k, _)| k == &key) {
                *lsp = Lsp::start(language, cwd, file.as_deref())
                    .ok()
                    .map(|p| (key, p));
            }
            if let Some((_, server)) = lsp.as_mut() {
                match server.complete(&request.text, request.cursor) {
                    Ok(result) => {
                        items = result;
                        provider = format!("{} / language server", server.name);
                    }
                    Err(e) => {
                        provider = format!("Language server unavailable: {e}");
                        *lsp = None;
                    }
                }
            }
            if items.is_empty() {
                items = code_candidates(&request.text, &word, start, request.cursor, language);
                provider = format!("{language} / vocabulary and document symbols (fallback)");
            }
        }
    }
    Answer {
        id: request.id,
        items,
        provider,
    }
}
fn executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(windows)]
    {
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into())
            .split(';')
            .any(|x| x.trim_start_matches('.').eq_ignore_ascii_case(&ext))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
}
#[cfg(windows)]
pub fn verify_native(shell: &str, cwd: &Path) -> anyhow::Result<()> {
    let line = "Get-Chil";
    let answer = complete(
        Request {
            id: 1,
            text: line.into(),
            cursor: line.len(),
            context: Context::Commands {
                shell: shell.into(),
                cwd: cwd.into(),
                environment: None,
            },
        },
        &mut None,
    );
    anyhow::ensure!(
        answer
            .items
            .iter()
            .any(|c| c.insert.eq_ignore_ascii_case("Get-ChildItem")),
        "Native completion failed: {}",
        answer.provider
    );
    Ok(())
}
#[cfg(target_os = "linux")]
pub fn verify_linux(cwd: &Path) -> anyhow::Result<()> {
    let answer = complete(
        Request {
            id: 1,
            text: "prin".into(),
            cursor: 4,
            context: Context::Commands {
                shell: "/bin/bash".into(),
                cwd: cwd.into(),
                environment: None,
            },
        },
        &mut None,
    );
    anyhow::ensure!(
        answer.items.iter().any(|c| c.insert == "printf"),
        "Linux PATH completion failed: {}",
        answer.provider
    );
    Ok(())
}
fn find_program(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|p| {
            if cfg!(windows) {
                vec![
                    p.join(name),
                    p.join(format!("{name}.exe")),
                    p.join(format!("{name}.cmd")),
                ]
            } else {
                vec![p.join(name)]
            }
        })
        .find(|p| executable(p))
}
fn command_candidates(
    line: &str,
    word: &str,
    start: usize,
    end: usize,
    shell: &str,
    cwd: &Path,
) -> Vec<Candidate> {
    let command_position =
        line[..start].trim().is_empty() || line[..start].trim_end().ends_with(['|', ';', '&']);
    let mut names = BTreeSet::new();
    let needle = word.to_lowercase();
    if command_position {
        for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if executable(&p) {
                        let name = if cfg!(windows) {
                            p.file_stem()
                        } else {
                            p.file_name()
                        };
                        if let Some(name) = name {
                            names.insert(name.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        let builtins = if shell.to_lowercase().contains("cmd") {
            vec![
                "cd", "cls", "copy", "del", "dir", "echo", "exit", "for", "if", "mkdir", "move",
                "ren", "set", "type", "where",
            ]
        } else {
            vec![
                "cd", "echo", "exit", "export", "alias", "pwd", "read", "set", "source", "type",
                "unset",
            ]
        };
        for name in builtins {
            names.insert(name.into());
        }
    }
    let mut items: Vec<_> = names
        .into_iter()
        .filter(|n| n.to_lowercase().starts_with(&needle))
        .take(80)
        .map(|n| candidate(n.clone(), n, "command", start, end))
        .collect();
    let expanded = if word.starts_with('~') {
        format!(
            "{}{}",
            std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_default(),
            &word[1..]
        )
    } else {
        word.into()
    };
    let path = Path::new(&expanded);
    let trailing = word.ends_with(['/', '\\']);
    let parent = if trailing {
        path
    } else {
        path.parent().unwrap_or(Path::new(""))
    };
    let folder = if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        cwd.join(parent)
    };
    let partial = if trailing {
        "".into()
    } else {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    };
    if let Ok(entries) = std::fs::read_dir(folder) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.to_lowercase().starts_with(&partial.to_lowercase()) {
                let lead = &word[..word.len() - partial.len()];
                let mut insert = format!("{lead}{name}");
                if e.path().is_dir() {
                    insert.push(std::path::MAIN_SEPARATOR);
                }
                if insert.contains(' ') && !line[..start].ends_with(['\'', '"']) {
                    insert = format!("\"{insert}\"");
                }
                items.push(candidate(
                    name,
                    insert,
                    if e.path().is_dir() { "folder" } else { "file" },
                    start,
                    end,
                ));
            }
        }
    }
    items.truncate(100);
    items
}
fn code_candidates(
    text: &str,
    word: &str,
    start: usize,
    end: usize,
    language: &str,
) -> Vec<Candidate> {
    let keywords=match language{"rust"=>"fn let mut pub use impl struct enum match if else for in while return self Self trait async await move Option Result Some None Ok Err String Vec println! format!", "python"=>"def class import from as if elif else for in while return self None True False try except finally with async await lambda print len range str list dict", "javascript"|"typescript"=>"const let var function return if else for of in class extends import from export default async await new this console.log Promise interface type string number boolean", "json"=>"true false null", "powershell"=>"Get-ChildItem Get-Command Get-Help Get-Content Set-Content Select-Object Where-Object ForEach-Object Write-Output function param foreach if else try catch finally return $true $false $null", "ui"=>"nav bx hdr ftr row col ico in list tbl code at t h p gap", _=>"if then else fi for in do done case esac while function export local return echo printf"};
    let mut words: BTreeSet<String> = keywords.split_whitespace().map(str::to_string).collect();
    for symbol in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if symbol.len() > 1 {
            words.insert(symbol.into());
        }
    }
    words
        .into_iter()
        .filter(|s| s != word && s.to_lowercase().starts_with(&word.to_lowercase()))
        .take(80)
        .map(|s| candidate(s.clone(), s, "symbol", start, end))
        .collect()
}
pub(crate) fn output_timeout(mut command: Command, timeout: Duration) -> anyhow::Result<Vec<u8>> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let out = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut b = vec![];
        let _ = out.take(1024 * 1024).read_to_end(&mut b);
        let _ = tx.send(b);
    });
    let until = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            anyhow::ensure!(status.success(), "Provider exited with {status}");
            return Ok(rx.recv_timeout(Duration::from_secs(1))?);
        }
        if Instant::now() > until {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Completion provider timed out");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn utf16_offset(text: &str, units: usize) -> usize {
    let mut n = 0;
    for (i, c) in text.char_indices() {
        if n >= units {
            return i;
        }
        n += c.len_utf16();
    }
    text.len()
}
fn position(text: &str, byte: usize) -> Value {
    let before = &text[..byte];
    json!({"line":before.bytes().filter(|b|*b==b'\n').count(),"character":before.rsplit('\n').next().unwrap_or("").encode_utf16().count()})
}
fn offset(text: &str, p: &Value) -> usize {
    let line = p["line"].as_u64().unwrap_or(0) as usize;
    let col = p["character"].as_u64().unwrap_or(0) as usize;
    let base = text
        .split_inclusive('\n')
        .take(line)
        .map(str::len)
        .sum::<usize>()
        .min(text.len());
    base + utf16_offset(text[base..].split('\n').next().unwrap_or(""), col)
}
struct Lsp {
    child: Child,
    input: ChildStdin,
    rx: Receiver<Value>,
    id: u64,
    version: u64,
    uri: String,
    language: String,
    pub name: String,
    opened: bool,
}
impl Lsp {
    fn start(language: &str, cwd: &Path, file: Option<&Path>) -> anyhow::Result<Self> {
        let options: Vec<(&str, Vec<&str>)> = match language {
            "rust" => vec![("rust-analyzer", vec![])],
            "python" => vec![("pylsp", vec![]), ("pyright-langserver", vec!["--stdio"])],
            "javascript" | "typescript" => vec![("typescript-language-server", vec!["--stdio"])],
            "json" => vec![("vscode-json-language-server", vec!["--stdio"])],
            _ => vec![],
        };
        let (program, args) = options
            .into_iter()
            .find_map(|(name, args)| find_program(name).map(|p| (p, args)))
            .ok_or_else(|| anyhow::anyhow!("No installed language server"))?;
        let name = program.display().to_string();
        let mut child = Command::new(&program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut length = 0usize;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 {
                        return;
                    }
                    if h.trim().is_empty() {
                        break;
                    }
                    if let Some((_, s)) = h
                        .split_once(':')
                        .filter(|(key, _)| key.eq_ignore_ascii_case("Content-Length"))
                    {
                        length = s.trim().parse().unwrap_or(0);
                    }
                }
                if length == 0 || length > 4 * 1024 * 1024 {
                    return;
                }
                let mut b = vec![0; length];
                if reader.read_exact(&mut b).is_err() {
                    return;
                }
                if let Ok(v) = serde_json::from_slice(&b) {
                    if tx.send(v).is_err() {
                        return;
                    }
                }
            }
        });
        let scratch = cwd.join(match language {
            "rust" => "scratch.rs",
            "python" => "scratch.py",
            "json" => "scratch.json",
            "typescript" => "scratch.ts",
            _ => "scratch.js",
        });
        let uri = file_uri(file.unwrap_or(&scratch));
        let mut l = Self {
            child,
            input,
            rx,
            id: 0,
            version: 0,
            uri,
            language: language.into(),
            name,
            opened: false,
        };
        let root = file_uri(cwd);
        l.request("initialize",json!({"processId":std::process::id(),"rootUri":root,"capabilities":{"textDocument":{"completion":{"completionItem":{"snippetSupport":false}}}}}))?;
        l.notify("initialized", json!({}))?;
        Ok(l)
    }
    fn write(&mut self, value: Value) -> anyhow::Result<()> {
        let b = serde_json::to_vec(&value)?;
        write!(self.input, "Content-Length: {}\r\n\r\n", b.len())?;
        self.input.write_all(&b)?;
        self.input.flush()?;
        Ok(())
    }
    fn notify(&mut self, method: &str, params: Value) -> anyhow::Result<()> {
        self.write(json!({"jsonrpc":"2.0","method":method,"params":params}))
    }
    fn request(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.id += 1;
        let id = self.id;
        self.write(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let v = self
                .rx
                .recv_timeout(until.saturating_duration_since(Instant::now()))?;
            if v["id"] == json!(id) {
                if !v["error"].is_null() {
                    anyhow::bail!("{}", v["error"]);
                }
                return Ok(v["result"].clone());
            }
            if v.get("method").is_some() && v.get("id").is_some() {
                self.write(json!({"jsonrpc":"2.0","id":v["id"],"result":null}))?;
            }
        }
    }
    fn complete(&mut self, text: &str, cursor: usize) -> anyhow::Result<Vec<Candidate>> {
        self.version += 1;
        if !self.opened {
            self.notify("textDocument/didOpen",json!({"textDocument":{"uri":self.uri,"languageId":self.language,"version":self.version,"text":text}}))?;
            self.opened = true;
        } else {
            self.notify("textDocument/didChange",json!({"textDocument":{"uri":self.uri,"version":self.version},"contentChanges":[{"text":text}]}))?;
        }
        let result=self.request("textDocument/completion",json!({"textDocument":{"uri":self.uri},"position":position(text,cursor),"context":{"triggerKind":1}}))?;
        let entries = result.as_array().or_else(|| result["items"].as_array());
        let (start, _) = prefix(text, cursor);
        let mut out = vec![];
        if let Some(items) = entries {
            for v in items.iter().take(100) {
                let Some(label) = v["label"].as_str() else {
                    continue;
                };
                let edit = &v["textEdit"];
                let insert = edit["newText"]
                    .as_str()
                    .or_else(|| v["insertText"].as_str())
                    .unwrap_or(label);
                let range = if edit["range"].is_object() {
                    &edit["range"]
                } else {
                    &edit["replace"]
                };
                let (a, b) = if range.is_object() {
                    (offset(text, &range["start"]), offset(text, &range["end"]))
                } else {
                    (start, cursor)
                };
                out.push(candidate(
                    label.into(),
                    plain_snippet(insert),
                    v["detail"].as_str().unwrap_or(""),
                    a,
                    b,
                ));
            }
        }
        Ok(out)
    }
}
impl Drop for Lsp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn plain_snippet(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            if chars.peek() == Some(&'{') {
                chars.next();
                let mut inner = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    inner.push(c);
                }
                if let Some((_, default)) = inner.split_once(':') {
                    out.push_str(default);
                }
            } else if chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                    chars.next();
                }
            } else {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_positions() {
        let t = "a😀b\nfoo";
        assert_eq!(position(t, 5), json!({"line":0,"character":3}));
        assert_eq!(offset(t, &json!({"line":0,"character":3})), 5);
    }
    #[test]
    fn language_candidates() {
        assert!(code_candidates("pri", "pri", 0, 3, "python")
            .iter()
            .any(|x| x.insert == "print"));
        assert!(code_candidates("Str", "Str", 0, 3, "rust")
            .iter()
            .any(|x| x.insert == "String"));
    }
    #[test]
    fn os_commands() {
        let v = command_candidates("ec", "ec", 0, 2, "cmd", Path::new("."));
        assert!(v.iter().any(|x| x.insert == "echo"));
    }
    #[test]
    fn snippets() {
        assert_eq!(plain_snippet("call(${1:value})$0"), "call(value)");
    }
}

fn file_uri(path: &Path) -> String {
    let path = crate::history::display_path(path).replace('\\', "/");
    let mut encoded = String::new();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    format!("file:///{}", encoded.trim_start_matches('/'))
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn midline_replacement_sets_caret() {
        let mut c = Completion::default();
        c.items = vec![Candidate {
            label: "cafe".into(),
            insert: "cafe".into(),
            detail: String::new(),
            start: 5,
            end: 7,
        }];
        let mut text = "echo ca suffix".to_string();
        assert!(c.apply(&mut text));
        assert_eq!(text, "echo cafe suffix");
        assert_eq!(c.applied_cursor, Some(9));
    }
    #[test]
    fn file_uri_escapes_reserved_and_unicode() {
        let uri = file_uri(Path::new("/project/a #?%é.rs"));
        assert!(uri.contains("a%20%23%3F%25%C3%A9.rs"));
    }
}
