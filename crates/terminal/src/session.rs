use anyhow::Result;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        mpsc::{self, SyncSender},
        Arc, Mutex, OnceLock,
    },
    thread,
};

#[derive(Default)]
pub struct Replies {
    pending: Vec<Vec<u8>>,
    context: Option<(String, String)>,
    ui_bytes: Vec<u8>,
    ui_update: Option<Option<terminal_ui_kit::View>>,
}
impl vt100::Callbacks for Replies {
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        if params.first() == Some(&b"777".as_slice()) {
            match params.get(1).copied() {
                Some(b"ui-begin") => self.ui_bytes.clear(),
                Some(b"ui-part") => {
                    if let Some(bytes) = params.get(2).and_then(|p| crate::inline_ui::decode_hex(p))
                    {
                        if self.ui_bytes.len() + bytes.len() <= terminal_ui_kit::MAX_MESSAGE_BYTES {
                            self.ui_bytes.extend(bytes);
                        } else {
                            self.ui_bytes.clear();
                        }
                    }
                }
                Some(b"ui-end") => {
                    if let Ok(view) = terminal_ui_kit::decode(&self.ui_bytes) {
                        self.ui_update = Some(Some(view));
                    }
                    self.ui_bytes.clear();
                }
                Some(b"ui-close") => self.ui_update = Some(None),
                _ => {}
            }

            if params.get(1) == Some(&b"context".as_slice()) && params.len() >= 4 {
                self.context = Some((
                    crate::shell_tools::decode(params[2]),
                    crate::shell_tools::decode(params[3]),
                ));
            }
            if params.get(1) == Some(&b"cwd".as_slice()) && params.len() >= 3 {
                self.context = Some((
                    String::from_utf8_lossy(params[2]).into_owned(),
                    String::new(),
                ));
            }
        }
    }
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        _: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        let n = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        if c == 'n' && n == 6 {
            let (r, col) = screen.cursor_position();
            self.pending
                .push(format!("\x1b[{};{}R", r + 1, col + 1).into_bytes());
        } else if c == 'n' && n == 5 {
            self.pending.push(b"\x1b[0n".to_vec());
        } else if c == 'c' && i1.is_none() {
            self.pending.push(b"\x1b[?1;2c".to_vec());
        }
    }
}

pub struct Session {
    pub cwd: std::path::PathBuf,
    pub shell: String,
    pub environment: Option<std::path::PathBuf>,
    pub ui_view: Option<terminal_ui_kit::View>,
    pub ui_surface: crate::ui_runtime::Surface,
    pub interaction: crate::terminal_state::State,
    backend: Backend,
    pub output: tokio::sync::mpsc::Receiver<Vec<u8>>,
    pub parser: vt100::Parser<Replies>,
    size: (u16, u16),
    pub exited: bool,
    wake: Wake,
}
type Wake = Arc<Mutex<Option<eframe::egui::Context>>>;

enum Backend {
    Local {
        master: Box<dyn MasterPty + Send>,
        child: Box<dyn Child + Send + Sync>,
        writer: SyncSender<Vec<u8>>,
    },
    Connection {
        task: tokio::task::JoinHandle<()>,
        writer: tokio::sync::mpsc::Sender<Vec<u8>>,
    },
}

pub(crate) struct ConnectionOutput {
    sender: tokio::sync::mpsc::Sender<Vec<u8>>,
    wake: Wake,
}
impl ConnectionOutput {
    pub async fn send(&self, bytes: Vec<u8>) -> Result<()> {
        self.sender.send(bytes).await?;
        self.repaint();
        Ok(())
    }
    pub async fn line(&self, text: &str) -> Result<()> {
        self.send(format!("{}\r\n", text.replace('\n', "\r\n")).into_bytes())
            .await
    }
    pub fn progress(&self, text: &str) {
        let _ = self.sender.try_send(format!("{text}\r\n").into_bytes());
        self.repaint();
    }
    fn repaint(&self) {
        if let Some(ctx) = self.wake.lock().unwrap().as_ref() {
            ctx.request_repaint();
        }
    }
}

impl Session {
    pub fn start(shell: &str, args: &[String], cwd: &Path) -> Result<Self> {
        let pair = native_pty_system().openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut command = CommandBuilder::new(shell);
        command.args(crate::shell_tools::arguments(shell, args));
        command.env("PATH", crate::shell_tools::path(None));
        if cfg!(windows)
            && std::path::Path::new(shell)
                .file_stem()
                .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("cmd"))
        {
            #[cfg(windows)]
            if let Some(hook) = crate::shell_tools::cmd_conda_hook(args) {
                // CMD expands the quoted path after ConPTY has serialized the arguments.
                let mut quoted = std::ffi::OsString::from("\"");
                quoted.push(hook);
                quoted.push("\"");
                command.env("DTERM_CONDA_HOOK", quoted);
                command.args(["/K", "%DTERM_CONDA_HOOK%"]);
            }
            let prompt = std::env::var("PROMPT").unwrap_or_else(|_| "$P$G".into());
            command.env("PROMPT", format!("$E]777;cwd;$P$E\\{prompt}"));
        }
        command.cwd(std::path::Path::new(&crate::history::display_path(cwd)));
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERMINAL_UI", "1");
        command.env("DTERM", "1");
        if let Ok(exe) = std::env::current_exe() {
            command.env("DTERM_EXE", &exe);
            command.env("TERMINAL_UI_EXE", exe);
        }
        let child = pair.slave.spawn_command(command)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer_pipe = pair.master.take_writer()?;
        let (writer, input) = mpsc::sync_channel::<Vec<u8>>(64);
        thread::spawn(move || {
            for bytes in input {
                if writer_pipe
                    .write_all(&bytes)
                    .and_then(|_| writer_pipe.flush())
                    .is_err()
                {
                    break;
                }
            }
        });
        let wake = std::sync::Arc::new(std::sync::Mutex::new(None::<eframe::egui::Context>));
        let notify = wake.clone();
        let (tx, output) = tokio::sync::mpsc::channel(64);
        thread::spawn(move || {
            let mut bytes = [0u8; 8192];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) | Err(_) => {
                        if let Some(ctx) = notify.lock().unwrap().as_ref() {
                            ctx.request_repaint();
                        }
                        break;
                    }
                    Ok(n) => {
                        if tx.blocking_send(bytes[..n].to_vec()).is_err() {
                            break;
                        }
                        if let Some(ctx) = notify.lock().unwrap().as_ref() {
                            ctx.request_repaint();
                        }
                    }
                }
            }
        });
        Ok(Self::new(
            shell,
            cwd,
            Backend::Local {
                master: pair.master,
                child,
                writer,
            },
            output,
            wake,
        ))
    }

    pub fn connect(args: &[String], cwd: &Path) -> Result<Self> {
        static RUNTIME: OnceLock<std::io::Result<tokio::runtime::Runtime>> = OnceLock::new();
        let runtime = RUNTIME
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
            })
            .as_ref()
            .map_err(|error| anyhow::anyhow!("Starting connection runtime: {error}"))?;
        let (writer, input) = tokio::sync::mpsc::channel(64);
        let (sender, output) = tokio::sync::mpsc::channel(64);
        let wake = Wake::default();
        let stream = ConnectionOutput {
            sender,
            wake: wake.clone(),
        };
        let args = args.to_vec();
        let task = runtime.spawn(async move {
            let _ = stream.line("Opening connection…").await;
            if let Err(error) = crate::connections::worker::run(args, input, &stream).await {
                let _ = stream.line(&format!("Error: {error:#}")).await;
            }
            let wake = stream.wake.clone();
            drop(stream);
            if let Some(ctx) = wake.lock().unwrap().as_ref() {
                ctx.request_repaint();
            };
        });
        Ok(Self::new(
            "Connections",
            cwd,
            Backend::Connection { task, writer },
            output,
            wake,
        ))
    }

    fn new(
        shell: &str,
        cwd: &Path,
        backend: Backend,
        output: tokio::sync::mpsc::Receiver<Vec<u8>>,
        wake: Wake,
    ) -> Self {
        let environment = matches!(backend, Backend::Local { .. })
            .then(|| std::env::var_os("CONDA_PREFIX").map(std::path::PathBuf::from))
            .flatten();
        Self {
            wake,
            cwd: cwd.into(),
            shell: shell.into(),
            environment,
            ui_view: None,
            ui_surface: Default::default(),
            interaction: Default::default(),
            backend,
            output,
            parser: vt100::Parser::new_with_callbacks(24, 80, 2000, Replies::default()),
            size: (24, 80),
            exited: false,
        }
    }
    pub fn terminate(&mut self) {
        if self.exited {
            return;
        }
        let Backend::Local { child, .. } = &mut self.backend else {
            if let Backend::Connection { task, .. } = &self.backend {
                task.abort();
            }
            self.exited = true;
            return;
        };
        #[cfg(windows)]
        if let Some(pid) = child.process_id() {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("taskkill.exe")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .creation_flags(0x08000000)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        #[cfg(unix)]
        if let Some(pid) = child.process_id() {
            let _ = std::process::Command::new("kill")
                .args(["-TERM", "--", &format!("-{pid}")])
                .status();
        }
        let _ = child.kill();
        let _ = child.wait();
        self.exited = true;
    }
    pub fn send(&mut self, bytes: &[u8]) -> Result<()> {
        anyhow::ensure!(
            bytes.len() <= 256 * 1024,
            "Input is larger than 256 KiB; split the paste into smaller parts"
        );
        match &self.backend {
            Backend::Local { writer, .. } => writer.try_send(bytes.to_vec()).map_err(|error| {
                anyhow::anyhow!("Terminal input is busy or disconnected: {error}")
            }),
            Backend::Connection { writer, .. } => {
                writer.try_send(bytes.to_vec()).map_err(|error| {
                    anyhow::anyhow!("Connection input is busy or disconnected: {error}")
                })
            }
        }
    }
    pub fn set_context(&self, ctx: &eframe::egui::Context) {
        *self.wake.lock().unwrap() = Some(ctx.clone());
    }
    pub fn poll(&mut self) {
        #[cfg(target_os = "linux")]
        if let Backend::Local { child, .. } = &self.backend {
            if let Some(pid) = child.process_id() {
                if let Ok(cwd) = std::fs::read_link(format!("/proc/{pid}/cwd")) {
                    self.cwd = cwd;
                }
            }
        }
        for _ in 0..64 {
            match self.output.try_recv() {
                Ok(bytes) => {
                    self.interaction.selection = None;
                    self.parser.process(&bytes)
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    if matches!(self.backend, Backend::Connection { .. }) {
                        self.exited = true;
                    }
                    if !self.exited {
                        if let Some(ctx) = self.wake.lock().unwrap().as_ref() {
                            ctx.request_repaint_after(std::time::Duration::from_millis(100));
                        }
                    }
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            }
        }
        if let Some(view) = self.parser.callbacks_mut().ui_update.take() {
            self.ui_view = view;
        }
        if let Some((cwd, environment)) = self.parser.callbacks_mut().context.take() {
            self.ui_view = None;
            let path = std::path::PathBuf::from(cwd);
            if path.is_dir() {
                self.cwd = path;
            }
            self.environment = if environment.is_empty() {
                None
            } else {
                Some(environment.into())
            };
        }
        for reply in std::mem::take(&mut self.parser.callbacks_mut().pending) {
            let _ = self.send(&reply);
        }
        if let Backend::Local { child, .. } = &mut self.backend {
            if let Ok(Some(_)) = child.try_wait() {
                self.exited = true;
            }
        }
    }
    pub fn copy_text(&self) -> String {
        self.interaction
            .selected(self.parser.screen())
            .unwrap_or_else(|| self.parser.screen().contents())
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        if self.size != (rows, cols) {
            if let Backend::Local { master, .. } = &self.backend {
                master.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })?;
            }
            self.parser.screen_mut().set_size(rows, cols);
            self.size = (rows, cols);
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.terminate();
    }
}

pub fn default_shell() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }
}

#[cfg(test)]
#[test]
fn desktop_connection_session_round_trips_telnet() -> Result<()> {
    use std::{net::TcpListener, time::Duration};

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port().to_string();
    let server = thread::spawn(move || -> Result<()> {
        let (mut stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(4)))?;
        let mut line = [0; 6];
        stream.read_exact(&mut line)?;
        anyhow::ensure!(&line == b"PING\r\n", "Unexpected Telnet input: {line:?}");
        stream.write_all(b"LOCAL_PONG\r\n")?;
        Ok(())
    });
    let folder = tempfile::tempdir()?;
    let config = folder.path().join("settings.toml");
    let mut session = Session::connect(
        &[
            "telnet".into(),
            "127.0.0.1".into(),
            "--port".into(),
            port,
            "--config".into(),
            config.to_string_lossy().into_owned(),
        ],
        folder.path(),
    )?;
    session.send(b"PING\r")?;
    crate::wait_output(&mut session, "LOCAL_PONG")?;
    server.join().expect("Telnet test server panicked")?;
    Ok(())
}
