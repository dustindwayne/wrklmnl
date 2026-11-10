#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod app;
mod appearance;
mod bar_background;
mod branding;
mod bridge;
mod chrome;
mod completion;
mod connections;
mod custom_shells;
mod dock;
mod editor;
mod file_picker;
mod file_preview;
mod file_properties;
mod highlight;
mod history;
mod icons;
mod inline_ui;
mod input;
mod installed_icon;
mod keybindings;
mod launcher;
mod licenses;
mod media_player;
mod persistence;
mod pixel_surface;
mod preview_edit;
mod preview_formats;
mod project_completion;
mod scratchpad;
mod session;
mod shell_catalog;
mod shell_tools;
mod terminal;
mod terminal_command;
mod terminal_groups;
mod terminal_input;
mod terminal_state;
mod theme;
mod tiles;
mod ui_runtime;
mod widgets;
mod workspace_layout;
mod workspace_ui;
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
    path::PathBuf,
    time::{Duration, Instant},
};
use terminal_ui_kit::{Action, View};

#[derive(Clone)]
pub struct Options {
    pub cwd: PathBuf,
    pub session: bool,
    pub terminal: bool,
    pub open_file: Option<PathBuf>,
    pub shell: String,
    pub args: Vec<String>,
    pub state: PathBuf,
    pub light: bool,
    pub accent: [u8; 3],
    pub ui_program: Option<String>,
    pub ui_args: Vec<String>,
    pub demo: bool,
    pub document: Option<PathBuf>,
    pub appearance: bool,
    pub keybindings: bool,
    pub font: Option<String>,
    pub font_size: Option<f32>,
    pub data: BTreeMap<String, String>,
    pub check: Option<bool>,
    pub run_ui: Option<PathBuf>,
    pub embedded_source: Option<String>,
}
impl Options {
    fn parse() -> anyhow::Result<Self> {
        let mut o = Self {
            cwd: std::env::current_dir()?,
            session: false,
            terminal: false,
            open_file: None,
            shell: session::default_shell(),
            args: vec![],
            state: history::default_path(),
            light: false,
            accent: [182, 182, 178],
            ui_program: None,
            ui_args: vec![],
            demo: false,
            document: None,
            appearance: false,
            keybindings: false,
            font: None,
            font_size: None,
            data: BTreeMap::new(),
            check: None,
            run_ui: None,
            embedded_source: None,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--session" => {
                    o.cwd = PathBuf::from(
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("--session requires a directory"))?,
                    );
                    o.session = true;
                }
                "--shell" => {
                    o.shell = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--shell requires a program"))?
                }
                "--state-file" => {
                    o.state = PathBuf::from(
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("--state-file requires a path"))?,
                    )
                }
                "--light" => {
                    o.light = true;
                }
                "--set" => {
                    let item = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--set requires name=value"))?;
                    let (key, value) = item
                        .split_once('=')
                        .ok_or_else(|| anyhow::anyhow!("--set requires name=value"))?;
                    anyhow::ensure!(!key.is_empty(), "A binding name is required");
                    o.data.insert(key.into(), value.into());
                }
                "--accent" => {
                    let h = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--accent requires RRGGBB"))?;
                    let h = h.trim_start_matches('#');
                    anyhow::ensure!(
                        h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()),
                        "Accent must have six hexadecimal digits"
                    );
                    for i in 0..3 {
                        o.accent[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16)?;
                    }
                }
                "--ui-program" => {
                    o.ui_program = Some(
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("--ui-program requires a program"))?,
                    )
                }
                "--ui-arg" => o.ui_args.push(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--ui-arg requires a value"))?,
                ),
                "--demo" => o.demo = true,
                "--font" => {
                    o.font = Some(
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("--font requires a font file"))?,
                    )
                }
                "--font-size" => {
                    let size: f32 = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--font-size requires a number"))?
                        .parse()?;
                    anyhow::ensure!(
                        size.is_finite() && (10.0..=36.0).contains(&size),
                        "Font size must be 10-36"
                    );
                    o.font_size = Some(size);
                }
                "--data" => {
                    let path = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--data requires a JSON object file"))?;
                    o.data = serde_json::from_slice(&std::fs::read(path)?)?;
                }
                "--terminal" => o.terminal = true,
                "--appearance" => o.appearance = true,
                "--keybindings" => o.keybindings = true,
                "--check-ui" | "--emit-ui" => {
                    o.check = Some(arg == "--emit-ui");
                    o.document = Some(
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("Supply a UI source file"))?
                            .into(),
                    );
                }
                "--run-ui" => {
                    o.run_ui = Some(
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("--run-ui requires a source file"))?
                            .into(),
                    )
                }
                "--ui" => {
                    o.document =
                        Some(PathBuf::from(args.next().ok_or_else(|| {
                            anyhow::anyhow!("--ui requires a source file")
                        })?))
                }
                "--" => {
                    o.args.extend(args);
                    break;
                }
                "--help" => {
                    println!("WRKLMNL [FILE]\nOpen WRKLMNL to show the Dashboard.\nOpen a file with WRKLMNL to edit it.");
                    std::process::exit(0);
                }
                "--open" => {
                    o.open_file = Some(
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("--open requires a file"))?
                            .into(),
                    );
                }
                _ if !arg.starts_with('-') => {
                    anyhow::ensure!(o.open_file.is_none(), "Open one file per window");
                    o.open_file = Some(arg.into());
                }
                _ => anyhow::bail!("Unknown option {arg}; use --help"),
            }
        }
        o.cwd = o.cwd.canonicalize()?;
        anyhow::ensure!(o.cwd.is_dir(), "Working location must be a directory");
        Ok(o)
    }
}

pub fn demo_view(name: &str) -> View {
    View::new("hello", "Sample view")
        .text("Sample content for the UI integration check.")
        .heading(format!("Hello, {name}"))
        .input("name", "Name", name)
        .action("greet", "Update greeting")
        .table(
            vec!["Element".into(), "Support".into()],
            vec![
                vec!["Text, tables, inputs".into(), "Ready".into()],
                vec!["Actions, progress, code".into(), "Ready".into()],
            ],
        )
        .progress("Example progress", 0.65)
        .code(
            "powershell",
            "Get-ChildItem -Path $HOME | Select-Object Name",
        )
}
fn demo_child() -> anyhow::Result<()> {
    demo_view("world").emit()?;
    for line in io::stdin().lock().lines() {
        let event: Action = serde_json::from_str(&line?)?;
        if event.version == 1 && event.view == "hello" && event.action == "greet" {
            demo_view(
                event
                    .values
                    .get("name")
                    .map(String::as_str)
                    .unwrap_or("world"),
            )
            .emit()?;
        }
    }
    Ok(())
}
fn self_check() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let cwd = std::env::current_dir()?;
    #[cfg(windows)]
    let mut pty = session::Session::start(
        "cmd.exe",
        &["/D", "/C", "echo", "PTY_OK"].map(str::to_string),
        &cwd,
    )?;
    #[cfg(not(windows))]
    let mut pty = session::Session::start(exe.to_str().unwrap(), &["--probe-child".into()], &cwd)?;
    pty.resize(30, 100)?;
    let until = Instant::now() + Duration::from_secs(8);
    while Instant::now() < until {
        pty.poll();
        if pty.parser.screen().contents().contains("PTY_OK") {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    anyhow::ensure!(
        pty.parser.screen().contents().contains("PTY_OK"),
        "PTY child output was not received (exited={}, screen={:?})",
        pty.exited,
        pty.parser.screen().contents()
    );
    let mut bridge = bridge::Bridge::start(exe.to_str().unwrap(), &["--demo-app".into()])?;
    let view = bridge
        .messages
        .recv_timeout(Duration::from_secs(8))?
        .map_err(anyhow::Error::msg)?;
    anyhow::ensure!(view.id == "hello", "Initial UI document missing");
    let mut values = BTreeMap::new();
    values.insert("name".into(), "Rust".into());
    bridge.send(&Action {
        version: 1,
        view: "hello".into(),
        action: "greet".into(),
        values,
    })?;
    let update = bridge
        .messages
        .recv_timeout(Duration::from_secs(8))?
        .map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        serde_json::to_string(&update)?.contains("Hello, Rust"),
        "UI action round-trip failed"
    );
    dock::Dock::verify(&exe, &cwd)?;
    #[cfg(windows)]
    {
        completion::verify_native("powershell.exe", &cwd)?;
        let args = [
            "-NoLogo",
            "-NoProfile",
            "-NoExit",
            "-Command",
            "Write-Output 'INTERRUPT_WAIT'; Start-Sleep -Seconds 60",
        ]
        .map(str::to_string);
        let mut shell = session::Session::start("powershell.exe", &args, &cwd)?;
        wait_output(&mut shell, "INTERRUPT_WAIT")?;
        shell.send(&[3])?;
        std::thread::sleep(Duration::from_millis(200));
        shell.send(b"Write-Output ('INTERRUPT_'+'OK')\r")?;
        wait_output(&mut shell, "INTERRUPT_OK")?;
        anyhow::ensure!(!shell.exited, "Interrupt terminated the shell");
        shell.send(b"Writ-O")?;
        std::thread::sleep(Duration::from_millis(100));
        shell.poll();
        let mut line = terminal_command::Line {
            valid: true,
            text: "Writ-O".into(),
            cursor: 6,
            ..Default::default()
        };
        let completed = "Write-Output ('INLINE_'+'OK')";
        shell.send(
            &line
                .replacement("Writ-O", completed, completed.len())
                .unwrap(),
        )?;
        shell.send(b"\r")?;
        wait_output(&mut shell, "INLINE_OK")?;
        println!("PASS: inline completion edits the real terminal command line");
        println!("PASS: native PowerShell command completion and interrupting a running command");
        if shell_tools::conda().is_some() {
            let mut interactive =
                session::Session::start("powershell.exe", &["-NoProfile".into()], &cwd)?;
            interactive
                .send(b"conda activate base\rWrite-Output ('CONDA_'+'READY')\rgh --version\r")?;
            wait_output(&mut interactive, "CONDA_READY")?;
            wait_output(&mut interactive, "gh version")?;
            let until = Instant::now() + Duration::from_secs(8);
            while Instant::now() < until && interactive.environment.is_none() {
                interactive.poll();
                std::thread::sleep(Duration::from_millis(20));
            }
            anyhow::ensure!(
                interactive.environment.is_some(),
                "Conda environment metadata missing: {}",
                interactive.parser.screen().contents()
            );
            println!("PASS: interactive Conda activation, environment tracking, and GitHub CLI execution");
        }
    }
    #[cfg(target_os = "linux")]
    {
        completion::verify_linux(&cwd)?;
        let bash = shell_tools::find("bash")
            .ok_or_else(|| anyhow::anyhow!("Bash is required for Linux self-check"))?;
        let mut shell = session::Session::start(bash.to_str().unwrap(), &[], &cwd)?;
        shell.send(b"printf '%s%s\\n' 'LINUX_' 'READY'\r")?;
        wait_output(&mut shell, "LINUX_READY")?;
        shell.send(b"cd /tmp; printf '%s%s\\n' 'CWD_' 'READY'\r")?;
        wait_output(&mut shell, "CWD_READY")?;
        anyhow::ensure!(
            shell.cwd == std::path::Path::new("/tmp"),
            "Linux shell cwd was not tracked: {}",
            shell.cwd.display()
        );
        shell.send(b"printf '%s%s\\n' 'INTERRUPT_' 'WAIT'; sleep 60\r")?;
        wait_output(&mut shell, "INTERRUPT_WAIT")?;
        std::thread::sleep(Duration::from_millis(100));
        shell.send(&[3])?;
        shell.send(b"printf '%s%s\\n' 'INTERRUPT_' 'OK'\r")?;
        wait_output(&mut shell, "INTERRUPT_OK")?;
        anyhow::ensure!(!shell.exited, "Interrupt terminated Bash");
        if shell_tools::conda().is_some() {
            shell.send(b"conda activate base; printf '%s%s\\n' 'CONDA_' 'READY'\r")?;
            wait_output(&mut shell, "CONDA_READY")?;
            let until = Instant::now() + Duration::from_secs(8);
            while shell.environment.is_none() && Instant::now() < until {
                shell.poll();
                std::thread::sleep(Duration::from_millis(20));
            }
            anyhow::ensure!(
                shell.environment.is_some(),
                "Linux Conda activation metadata missing"
            );
            println!("PASS: Linux Conda activation and environment tracking");
        }
        println!("PASS: Linux PATH completion, real Bash input, cwd tracking and Ctrl+C recovery");
    }
    println!("PASS: real PTY spawn/output/resize, two-way UI protocol, and input after splitting/detaching/redocking");
    Ok(())
}
#[cfg(any(test, windows, target_os = "linux"))]
fn wait_output(shell: &mut session::Session, marker: &str) -> anyhow::Result<()> {
    let until = Instant::now() + Duration::from_secs(8);
    while Instant::now() < until {
        shell.poll();
        if shell.parser.screen().contents().contains(marker) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    anyhow::bail!(
        "Expected {marker}; screen: {}",
        shell.parser.screen().contents()
    )
}
pub const APP_NAME: &str = "WRKLMNL";

fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--licenses") {
        println!("{}", licenses::notices());
        return Ok(());
    }
    match std::env::args().nth(1).as_deref() {
        Some("--list-shells") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&shell_catalog::discover())?
            );
            return Ok(());
        }
        Some("--probe-ui") => {
            return inline_ui::run(
                "hdr:\"Hello, {name}\" > in:name=world > \"Greet\"@greet",
                &BTreeMap::from([("name".into(), "world".into())]),
            )
        }
        Some("--probe-session") => {
            use std::io::BufRead;
            for line in std::io::stdin().lock().lines() {
                println!("ACK:{}", line?);
            }
            return Ok(());
        }
        Some("--probe-child") => {
            println!("\x1b[32mPTY_OK\x1b[0m");
            return Ok(());
        }
        Some("--demo-app") => return demo_child(),
        Some("--self-check") => return self_check(),
        _ => {}
    }
    let options = Options::parse()?;
    if let Some(path) = &options.run_ui {
        return inline_ui::run(&std::fs::read_to_string(path)?, &options.data);
    }
    if let Some(emit) = options.check {
        let path = options.document.as_ref().unwrap();
        let result = (|| -> Result<View, String> {
            let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let doc = terminal_ui_language::Document::parse(&source)?;
            let mut data = options.data.clone();
            data.entry("status".into()).or_insert("Ready".into());
            data.entry("recent".into()).or_default();
            doc.render(&data)
        })();
        match result {
            Ok(view) => {
                if emit {
                    println!("{}", serde_json::to_string(&view)?);
                } else {
                    println!("Valid UI definition");
                }
                return Ok(());
            }
            Err(message) => {
                let location = message
                    .strip_prefix("Line ")
                    .and_then(|s| s.split_once(": "))
                    .and_then(|(s, _)| s.split_once(':'));
                let (line, col) = location
                    .map(|(l, c)| (l.parse::<usize>().ok(), c.parse::<usize>().ok()))
                    .unwrap_or((None, None));
                eprintln!(
                    "{}",
                    serde_json::json!({"line":line,"col":col,"path":path,"message":message})
                );
                std::process::exit(2);
            }
        }
    }

    let native = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(branding::icon())
            .with_decorations(false)
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([720.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        native,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, options)))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

#[cfg(all(test, windows))]
#[test]
fn cmd_starts_with_the_installed_conda_hook() -> anyhow::Result<()> {
    if shell_tools::cmd_conda_hook(&[]).is_none() {
        return Ok(());
    }
    let mut shell = session::Session::start("cmd.exe", &[], &std::env::current_dir()?)?;
    shell.send(b"echo DTERM_LEVEL_%CONDA_SHLVL%_READY\r")?;
    let level = std::env::var("CONDA_SHLVL").unwrap_or_else(|_| "0".into());
    wait_output(&mut shell, &format!("DTERM_LEVEL_{level}_READY"))?;
    anyhow::ensure!(
        !shell
            .parser
            .screen()
            .contents()
            .contains("not recognized as an internal"),
        "CMD rejected a startup command"
    );
    Ok(())
}
