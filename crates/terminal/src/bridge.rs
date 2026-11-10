//! Dedicated UI process channel. Terminal output is never interpreted as UI actions.
use anyhow::Result;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};
use terminal_ui_kit::{Action, View, MAX_MESSAGE_BYTES};

pub struct Bridge {
    child: Child,
    input: SyncSender<Action>,
    pub messages: Receiver<Result<View, String>>,
    wake: std::sync::Arc<std::sync::Mutex<Option<eframe::egui::Context>>>,
}
impl Bridge {
    pub fn start(program: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut input_pipe = child.stdin.take().expect("piped stdin");
        let (input, actions) = mpsc::sync_channel::<Action>(16);
        thread::spawn(move || {
            for action in actions {
                let result = (|| -> Result<()> {
                    serde_json::to_writer(&mut input_pipe, &action)?;
                    input_pipe.write_all(b"\n")?;
                    input_pipe.flush()?;
                    Ok(())
                })();
                if result.is_err() {
                    break;
                }
            }
        });
        let output = child.stdout.take().expect("piped stdout");
        let wake = std::sync::Arc::new(std::sync::Mutex::new(None::<eframe::egui::Context>));
        let notify = wake.clone();
        let (tx, messages) = mpsc::sync_channel(16);
        thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut bytes = Vec::new();
                // Read a bounded line without allowing unbounded allocations.
                let result = loop {
                    match reader.fill_buf() {
                        Ok([]) => {
                            break if bytes.is_empty() {
                                None
                            } else {
                                Some(terminal_ui_kit::decode(&bytes))
                            }
                        }
                        Ok(buf) => {
                            let end = buf.iter().position(|b| *b == b'\n').map(|i| i + 1);
                            let n = end.unwrap_or(buf.len());
                            if bytes.len() + n > MAX_MESSAGE_BYTES {
                                break Some(Err(
                                    "UI message exceeds 256 KiB; channel stopped".into()
                                ));
                            }
                            bytes.extend_from_slice(&buf[..n]);
                            reader.consume(n);
                            if end.is_some() {
                                break Some(terminal_ui_kit::decode(&bytes));
                            }
                        }
                        Err(e) => break Some(Err(e.to_string())),
                    }
                };
                match result {
                    Some(message) => {
                        let bad = message.is_err();
                        if let Some(ctx) = notify.lock().unwrap().as_ref() {
                            ctx.request_repaint();
                        }
                        if tx.send(message).is_err() || bad {
                            break;
                        }
                    }
                    None => {
                        if let Some(ctx) = notify.lock().unwrap().as_ref() {
                            ctx.request_repaint();
                        }
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            wake,
            input,
            messages,
        })
    }
    pub fn set_context(&self, ctx: &eframe::egui::Context) {
        *self.wake.lock().unwrap() = Some(ctx.clone());
    }
    pub fn connected(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
    pub fn send(&mut self, event: &Action) -> Result<()> {
        anyhow::ensure!(self.connected(), "UI application disconnected");
        self.input
            .try_send(event.clone())
            .map_err(|e| anyhow::anyhow!("UI application is busy or disconnected: {e}"))?;
        Ok(())
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
