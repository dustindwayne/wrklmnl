use anyhow::{ensure, Context, Result};
use eframe::egui;
use serde::Deserialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

const FRAME: [usize; 2] = [640, 360];
const DEMUXERS: &str = "aac,aiff,avi,flac,matroska,webm,mov,mp4,m4a,m4v,mp3,mpeg,mpegts,ogg,wav";
type LatestFrame = Arc<Mutex<Option<Vec<u8>>>>;

pub struct Player {
    path: PathBuf,
    pending: Option<mpsc::Receiver<Result<MediaInfo>>>,
    media: Option<MediaInfo>,
    running: Option<Playback>,
    texture: Option<egui::TextureHandle>,
    position: f64,
    scrub: Option<f64>,
    status: String,
}

struct MediaInfo {
    duration: f64,
    video: Option<u32>,
    audio: bool,
}

struct Playback {
    started: Instant,
    frames: LatestFrame,
    video: Option<Child>,
    audio: Option<Child>,
}

impl Player {
    pub fn new(path: PathBuf, ctx: &egui::Context) -> Self {
        let (sender, pending) = mpsc::channel();
        let source = path.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(probe(&source));
            ctx.request_repaint();
        });
        Self {
            path,
            pending: Some(pending),
            media: None,
            running: None,
            texture: None,
            position: 0.0,
            scrub: None,
            status: String::new(),
        }
    }

    pub fn pause(&mut self) {
        if self.running.is_some() {
            self.position = self.current_position();
            self.running = None;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.poll(ui.ctx());
        let Some(info) = &self.media else {
            if self.pending.is_some() {
                ui.weak("Reading media…");
            } else {
                ui.colored_label(ui.visuals().error_fg_color, &self.status);
            }
            return;
        };
        let duration = info.duration;
        #[cfg(feature = "capture")]
        if self.position == 0.0
            && self.running.is_none()
            && self.status.is_empty()
            && std::env::var_os("TERMINAL_CAPTURE_TO").is_some()
            && std::env::var_os("DTERM_CAPTURE_MEDIA_PLAY").is_some()
        {
            self.start(ui.ctx());
        }
        let info = self.media.as_ref().expect("media details loaded");
        if info.video.is_some() {
            let width = ui.available_width().min(720.0);
            let size = egui::vec2(width, width * 9.0 / 16.0);
            if let Some(texture) = &self.texture {
                ui.image((texture.id(), size));
            } else {
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 4, ui.visuals().extreme_bg_color);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Video preview",
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().weak_text_color(),
                );
            }
        } else {
            ui.weak("Audio playback");
        }

        ui.horizontal_wrapped(|ui| {
            let playing = self.running.is_some();
            if ui
                .add(
                    egui::Button::new(if playing { "Pause" } else { "Play" })
                        .fill(ui.visuals().selection.bg_fill),
                )
                .clicked()
            {
                if playing {
                    self.pause();
                } else {
                    self.start(ui.ctx());
                }
            }
            if crate::widgets::button(ui, "Stop").clicked() {
                self.running = None;
                self.position = 0.0;
                self.texture = None;
                self.scrub = None;
            }
            ui.weak(format!(
                "{} / {}",
                timestamp(self.scrub.unwrap_or_else(|| self.current_position())),
                timestamp(duration)
            ));
        });
        let mut cursor = self.scrub.unwrap_or_else(|| self.current_position());
        let slider = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = ui.available_width();
                ui.add(
                    egui::Slider::new(&mut cursor, 0.0..=duration)
                        .show_value(false)
                        .trailing_fill(true),
                )
                .on_hover_text("Seek")
            })
            .inner;
        if slider.dragged() {
            self.scrub = Some(cursor);
        } else if slider.drag_stopped() || slider.changed() {
            self.scrub = None;
            let restart = self.running.take().is_some();
            self.position = cursor;
            self.texture = None;
            if restart && cursor < duration {
                self.start(ui.ctx());
            }
        }
        if !self.status.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, &self.status);
        }
    }

    fn current_position(&self) -> f64 {
        let elapsed = self
            .running
            .as_ref()
            .map_or(0.0, |playing| playing.started.elapsed().as_secs_f64());
        self.media
            .as_ref()
            .map_or(0.0, |info| (self.position + elapsed).min(info.duration))
    }

    fn start(&mut self, ctx: &egui::Context) {
        let Some(info) = &self.media else {
            return;
        };
        if self.position >= info.duration {
            self.position = 0.0;
        }
        match Playback::start(&self.path, info, self.position, ctx) {
            Ok(playback) => {
                self.running = Some(playback);
                self.status.clear();
                ctx.request_repaint();
            }
            Err(error) => self.status = format!("Playback could not start: {error:#}"),
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(pending) = &self.pending {
            let result = match pending.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(anyhow::anyhow!("Media reader stopped")))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.pending = None;
                match result {
                    Ok(info) => self.media = Some(info),
                    Err(error) => self.status = format!("{error:#}"),
                }
            }
        }
        let Some(running) = &mut self.running else {
            return;
        };
        if let Some(frame) = running.frames.lock().unwrap().take() {
            let image = egui::ColorImage::from_rgba_unmultiplied(FRAME, &frame);
            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture =
                    Some(ctx.load_texture("media_frame", image, egui::TextureOptions::LINEAR));
            }
        }
        match running.finished() {
            Ok(true) => {
                self.position = self.media.as_ref().map_or(0.0, |info| info.duration);
                self.running = None;
            }
            Ok(false) => ctx.request_repaint_after(Duration::from_millis(40)),
            Err(error) => {
                self.pause();
                self.status = format!("Playback stopped: {error:#}");
            }
        }
    }
}

impl Playback {
    fn start(path: &Path, info: &MediaInfo, position: f64, ctx: &egui::Context) -> Result<Self> {
        let seek = format!("{position:.3}");
        let frames = LatestFrame::default();
        let mut playback = Self {
            started: Instant::now(),
            frames,
            video: None,
            audio: None,
        };
        if let Some(index) = info.video {
            let mapping = format!("0:{index}");
            let scale = format!(
                "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,fps=24",
                FRAME[0], FRAME[1], FRAME[0], FRAME[1],
            );
            let mut child = tool("ffmpeg")?
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-nostdin",
                    "-ss",
                    &seek,
                    "-re",
                ])
                .args(input_limits())
                .arg("-i")
                .arg(path)
                .args([
                    "-map", &mapping, "-an", "-sn", "-dn", "-vf", &scale, "-pix_fmt", "rgba",
                    "-threads", "1", "-f", "rawvideo", "pipe:1",
                ])
                .stdout(Stdio::piped())
                .spawn()
                .context("Starting the video decoder")?;
            let mut output = child.stdout.take().expect("piped video output");
            playback.video = Some(child);
            let latest = playback.frames.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let mut buffer = vec![0; FRAME[0] * FRAME[1] * 4];
                while output.read_exact(&mut buffer).is_ok() {
                    let next = latest.lock().unwrap().replace(buffer);
                    buffer = next.unwrap_or_else(|| vec![0; FRAME[0] * FRAME[1] * 4]);
                    ctx.request_repaint();
                }
                ctx.request_repaint();
            });
        }
        if info.audio {
            playback.audio = Some(
                tool("ffplay")?
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-nodisp",
                        "-autoexit",
                        "-vn",
                        "-sn",
                        "-ss",
                        &seek,
                    ])
                    .args(input_limits())
                    .arg("-i")
                    .arg(path)
                    .spawn()
                    .context("Starting audio playback")?,
            );
        }
        playback.started = Instant::now();
        Ok(playback)
    }

    fn finished(&mut self) -> Result<bool> {
        let mut finished = true;
        for (child, name) in [
            (&mut self.video, "Video decoder"),
            (&mut self.audio, "Audio player"),
        ] {
            if let Some(process) = child {
                match process.try_wait()? {
                    Some(status) => {
                        ensure!(status.success(), "{name} exited with {status}");
                        *child = None;
                    }
                    None => finished = false,
                }
            }
        }
        Ok(finished)
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        for child in [self.video.as_mut(), self.audio.as_mut()]
            .into_iter()
            .flatten()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn input_limits() -> [&'static str; 8] {
    [
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        DEMUXERS,
        "-probesize",
        "4M",
        "-analyzeduration",
        "3000000",
    ]
}

fn tool(name: &str) -> Result<Command> {
    let search = std::env::var_os("PATH").unwrap_or_default();
    let program = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    let executable = std::env::split_paths(&search)
        .filter(|path| path.is_absolute())
        .map(|path| path.join(&program))
        .find(|path| path.is_file())
        .with_context(|| {
            format!("Install FFmpeg with {name} available on PATH for in-app playback.")
        })?;
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    Ok(command)
}

fn probe(path: &Path) -> Result<MediaInfo> {
    #[derive(Deserialize)]
    struct Probe {
        format: Format,
        streams: Vec<Stream>,
    }
    #[derive(Deserialize)]
    struct Format {
        duration: Option<String>,
    }
    #[derive(Deserialize)]
    struct Stream {
        index: u32,
        codec_type: Option<String>,
        disposition: Option<Disposition>,
    }
    #[derive(Deserialize)]
    struct Disposition {
        attached_pic: Option<u8>,
    }

    let output = tool("ffprobe")?
        .args(["-v", "error"])
        .args(input_limits())
        .args([
            "-show_entries",
            "format=duration:stream=index,codec_type:stream_disposition=attached_pic",
            "-of",
            "json",
            "-i",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .output()
        .context("Reading media details")?;
    ensure!(
        output.status.success(),
        "This media file could not be read."
    );
    let probe: Probe = serde_json::from_slice(&output.stdout)?;
    let duration: f64 = probe
        .format
        .duration
        .as_deref()
        .unwrap_or_default()
        .parse()
        .context("The media file has no usable duration")?;
    ensure!(
        duration.is_finite() && duration > 0.0,
        "The media file has no usable duration"
    );
    let video = probe
        .streams
        .iter()
        .find(|stream| {
            stream.codec_type.as_deref() == Some("video")
                && stream
                    .disposition
                    .as_ref()
                    .and_then(|value| value.attached_pic)
                    != Some(1)
        })
        .map(|stream| stream.index);
    let audio = probe
        .streams
        .iter()
        .any(|stream| stream.codec_type.as_deref() == Some("audio"));
    ensure!(video.is_some() || audio, "No playable audio or video track");
    Ok(MediaInfo {
        duration,
        video,
        audio,
    })
}

fn timestamp(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
