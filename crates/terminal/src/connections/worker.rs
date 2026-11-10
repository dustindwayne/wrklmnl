use super::{
    config, discovery,
    session::{self, Endpoint, Event},
};
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::{path::PathBuf, time::Duration};
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(version, about = "Serial, SSH, Telnet and SFTP")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Connect to a legacy cleartext Telnet service.
    Telnet {
        host: String,
        #[arg(long, default_value_t = 23)]
        port: u16,
        #[arg(long)]
        seconds: Option<u64>,
    },
    /// Open a serial port. Redirect stdin/stdout for binary I/O.
    Serial {
        port: String,
        #[arg(long)]
        baud: Option<u32>,
        #[arg(long)]
        data_bits: Option<u8>,
        #[arg(long, value_parser = ["none", "even", "odd"])]
        parity: Option<String>,
        #[arg(long)]
        stop_bits: Option<u8>,
        #[arg(long, value_parser = ["none", "software", "hardware"])]
        flow: Option<String>,
        #[arg(long)]
        seconds: Option<u64>,
    },
    /// Open a shell using a saved TOML profile or an explicit host and user.
    Ssh {
        #[arg(long, conflicts_with = "host")]
        profile: Option<String>,
        #[arg(long, required_unless_present = "profile")]
        host: Option<String>,
        #[arg(long, required_unless_present = "profile")]
        user: Option<String>,
        #[arg(long, default_value_t = 22)]
        port: u16,
        #[arg(long)]
        identity: Option<PathBuf>,
        #[arg(long)]
        known_hosts: Option<PathBuf>,
        /// Read a password or key passphrase from this environment variable.
        #[arg(long)]
        password_env: Option<String>,
        #[arg(long)]
        seconds: Option<u64>,
    },
    /// Upload a file through SFTP with automatic resume.
    Upload {
        #[arg(long)]
        profile: String,
        local: PathBuf,
        remote: String,
        #[arg(long, default_value_t = 5)]
        retries: u8,
        #[arg(long)]
        password_env: Option<String>,
    },
    /// Download a file through SFTP with automatic resume.
    Download {
        #[arg(long)]
        profile: String,
        remote: String,
        local: PathBuf,
        #[arg(long, default_value_t = 5)]
        retries: u8,
        #[arg(long)]
        password_env: Option<String>,
    },
}

pub async fn run(
    args: Vec<String>,
    input: mpsc::Receiver<Vec<u8>>,
    output: &super::super::session::ConnectionOutput,
) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("WRKLMNL connection".into()).chain(args))?;
    let path = cli.config.map_or_else(config::default_path, Ok)?;
    let command = cli.command;
    let direction = if matches!(command, Command::Upload { .. }) {
        super::transfer::Direction::Upload
    } else {
        super::transfer::Direction::Download
    };
    match command {
        Command::Upload {
            profile,
            local,
            remote,
            retries,
            password_env,
        }
        | Command::Download {
            profile,
            local,
            remote,
            retries,
            password_env,
        } => {
            let settings = config::Settings::load(&path)?;
            let ssh = settings
                .ssh
                .get(&profile)
                .with_context(|| format!("Unknown SSH profile {profile}"))?;
            let password = password_env
                .map(std::env::var)
                .transpose()
                .context("Reading password environment variable")?;
            super::transfer::run(
                ssh,
                password.as_deref(),
                direction,
                &local,
                &remote,
                retries,
                |message| output.progress(&message),
            )
            .await
        }
        Command::Telnet {
            host,
            port,
            seconds,
        } => {
            let mut settings = config::Settings::load(&path)?;
            terminal(
                Endpoint::Telnet(host, port),
                None,
                seconds,
                &mut settings,
                &path,
                input,
                output,
            )
            .await
        }
        Command::Serial {
            port,
            baud,
            data_bits,
            parity,
            stop_bits,
            flow,
            seconds,
        } => {
            let mut settings = config::Settings::load(&path)?;
            let mut serial = discovery::snapshot()
                .await
                .unwrap_or_default()
                .iter()
                .find(|d| d.port == port)
                .and_then(|d| settings.devices.get(&d.id))
                .unwrap_or(&settings.serial)
                .clone();
            if let Some(value) = baud {
                serial.baud = value;
            }
            if let Some(value) = data_bits {
                serial.data_bits = value;
            }
            if let Some(value) = parity {
                serial.parity = value;
            }
            if let Some(value) = stop_bits {
                serial.stop_bits = value;
            }
            if let Some(value) = flow {
                serial.flow = value;
            }
            terminal(
                Endpoint::Serial(port, serial),
                None,
                seconds,
                &mut settings,
                &path,
                input,
                output,
            )
            .await
        }
        Command::Ssh {
            profile,
            host,
            user,
            port,
            identity,
            known_hosts,
            password_env,
            seconds,
        } => {
            let mut settings = config::Settings::load(&path)?;
            let mut ssh = if let Some(name) = profile {
                settings
                    .ssh
                    .get(&name)
                    .cloned()
                    .with_context(|| format!("Unknown SSH profile {name}"))?
            } else {
                config::Ssh {
                    host: host.context("Specify --host")?,
                    user: user.context("Specify --user")?,
                    port,
                    identity: None,
                    known_hosts: None,
                }
            };
            if identity.is_some() {
                ssh.identity = identity;
            }
            if known_hosts.is_some() {
                ssh.known_hosts = known_hosts;
            }
            let password = password_env
                .map(|name| std::env::var(&name).with_context(|| format!("Reading {name}")))
                .transpose()?;
            terminal(
                Endpoint::Ssh(ssh),
                password,
                seconds,
                &mut settings,
                &path,
                input,
                output,
            )
            .await
        }
    }
}

async fn terminal(
    endpoint: Endpoint,
    password: Option<String>,
    seconds: Option<u64>,
    settings: &mut config::Settings,
    path: &std::path::Path,
    input: mpsc::Receiver<Vec<u8>>,
    output: &super::super::session::ConnectionOutput,
) -> Result<()> {
    let label = endpoint.label();
    let mut session = session::open(endpoint, password, 80, 24, input);
    let limit = async {
        if let Some(seconds) = seconds {
            tokio::time::sleep(Duration::from_secs(seconds)).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(limit);
    loop {
        tokio::select! {
            _ = &mut limit => return Ok(()),
            event = session.events.recv() => match event {
                Some(Event::Connected) => {
                    settings.remember(&label);
                    if let Err(error) = settings.save(path) {
                        tracing::warn!(%error, "Saving recent session failed");
                    }
                    output.line(&format!("Connected to {label}. Close this session to disconnect.")).await?;
                }
                Some(Event::Rx(bytes)) => output.send(bytes).await?,
                Some(Event::Failed(error)) => bail!("{error}"),
                Some(Event::Closed) | None => return Ok(()),
            }
        }
    }
}
