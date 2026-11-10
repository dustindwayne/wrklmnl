use super::{config, ssh};
use anyhow::{bail, ensure, Context, Result};
use std::time::Duration;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::mpsc,
};
use tokio_serial::SerialPortBuilderExt;

#[derive(Clone)]
pub enum Endpoint {
    Serial(String, config::Serial),
    Ssh(config::Ssh),
    Telnet(String, u16),
}

impl Endpoint {
    pub fn label(&self) -> String {
        match self {
            Self::Serial(port, _) => port.clone(),
            Self::Telnet(host, port) => format!("telnet://{host}:{port}"),
            Self::Ssh(p) => format!("{}@{}:{}", p.user, p.host, p.port),
        }
    }
}

pub enum Event {
    Connected,
    Rx(Vec<u8>),
    Closed,
    Failed(String),
}
pub struct Session {
    pub events: mpsc::Receiver<Event>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn open(
    endpoint: Endpoint,
    password: Option<String>,
    cols: u16,
    rows: u16,
    input: mpsc::Receiver<Vec<u8>>,
) -> Session {
    let (tx, events) = mpsc::channel(64);
    let task = tokio::spawn(async move {
        let event = match run(endpoint, password, cols, rows, input, &tx).await {
            Ok(()) => Event::Closed,
            Err(e) => Event::Failed(format!("{e:#}")),
        };
        let _ = tx.send(event).await;
    });
    Session { events, task }
}

async fn run(
    endpoint: Endpoint,
    password: Option<String>,
    cols: u16,
    rows: u16,
    input: mpsc::Receiver<Vec<u8>>,
    events: &mpsc::Sender<Event>,
) -> Result<()> {
    match endpoint {
        Endpoint::Serial(port, settings) => {
            let stream = tokio::task::spawn_blocking(move || {
                serial_builder(&port, &settings)?
                    .open_native_async()
                    .with_context(|| format!("Opening serial port {port}"))
            })
            .await??;
            duplex(stream, input, events).await
        }
        Endpoint::Telnet(host, port) => super::telnet::run(&host, port, input, events).await,
        Endpoint::Ssh(profile) => {
            let (_connection, channel) = tokio::time::timeout(
                Duration::from_secs(30),
                ssh::connect(&profile, password.as_deref(), cols, rows),
            )
            .await
            .context("SSH connection timed out")??;
            drop(password);
            duplex(channel.into_stream(), input, events).await
        }
    }
}

pub fn serial_builder(
    port: &str,
    settings: &config::Serial,
) -> Result<serialport::SerialPortBuilder> {
    use serialport::{DataBits::*, FlowControl, Parity, StopBits};
    ensure!(settings.baud > 0, "Baud must be positive");
    Ok(serialport::new(port, settings.baud)
        .data_bits(match settings.data_bits {
            5 => Five,
            6 => Six,
            7 => Seven,
            8 => Eight,
            _ => bail!("Data bits must be 5–8"),
        })
        .stop_bits(match settings.stop_bits {
            1 => StopBits::One,
            2 => StopBits::Two,
            _ => bail!("Stop bits must be 1 or 2"),
        })
        .parity(match settings.parity.as_str() {
            "none" => Parity::None,
            "even" => Parity::Even,
            "odd" => Parity::Odd,
            _ => bail!("Invalid parity"),
        })
        .flow_control(match settings.flow.as_str() {
            "none" => FlowControl::None,
            "software" => FlowControl::Software,
            "hardware" => FlowControl::Hardware,
            _ => bail!("Invalid flow control"),
        }))
}

async fn duplex(
    stream: impl AsyncRead + AsyncWrite + Unpin,
    mut input: mpsc::Receiver<Vec<u8>>,
    events: &mpsc::Sender<Event>,
) -> Result<()> {
    if events.send(Event::Connected).await.is_err() {
        return Ok(());
    }
    let (mut reader, mut writer) = tokio::io::split(stream);
    let receive = async {
        let mut buffer = [0; 4096];
        loop {
            let count = reader.read(&mut buffer).await?;
            if count == 0
                || events
                    .send(Event::Rx(buffer[..count].to_vec()))
                    .await
                    .is_err()
            {
                return Ok(());
            }
        }
    };
    let send = async {
        while let Some(bytes) = input.recv().await {
            writer.write_all(&bytes).await?;
            writer.flush().await?;
        }
        writer.shutdown().await?;
        std::future::pending::<Result<()>>().await
    };
    tokio::select! { result = receive => result, result = send => result }
}
