use super::{config::Ssh, ssh};
use anyhow::{ensure, Context, Result};
use russh_sftp::{
    client::{error::Error as SftpError, SftpSession},
    protocol::{OpenFlags, StatusCode},
};
use std::{
    io::{ErrorKind, SeekFrom},
    path::Path,
    time::{Duration, Instant},
};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt},
};

#[derive(Clone, Copy)]
pub enum Direction {
    Upload,
    Download,
}

pub async fn run(
    profile: &Ssh,
    password: Option<&str>,
    direction: Direction,
    local: &Path,
    remote: &str,
    retries: u8,
    mut progress: impl FnMut(String),
) -> Result<()> {
    ensure!(local.file_name().is_some(), "Specify a local file");
    ensure!(
        !remote.is_empty() && !remote.ends_with('/'),
        "Specify a remote file"
    );
    for attempt in 0..=retries {
        let result = once(
            profile,
            password,
            direction,
            local,
            remote,
            attempt > 0,
            &mut progress,
        )
        .await;
        match result {
            Ok(()) => return Ok(()),
            Err(e) if attempt < retries && retryable(&e) => {
                let delay = 1u64 << attempt.min(4);
                progress(format!(
                    "Connection interrupted ({e:#}). Resuming in {delay} s."
                ));
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

async fn once(
    profile: &Ssh,
    password: Option<&str>,
    direction: Direction,
    local: &Path,
    remote: &str,
    retrying: bool,
    progress: &mut impl FnMut(String),
) -> Result<()> {
    let (_connection, sftp) = connect(profile, password).await?;
    match direction {
        Direction::Upload => upload(&sftp, local, remote, retrying, progress).await,
        Direction::Download => download(&sftp, local, remote, progress).await,
    }
}

async fn connect(
    profile: &Ssh,
    password: Option<&str>,
) -> Result<(russh::client::Handle<ssh::Verifier>, SftpSession)> {
    let (_connection, sftp) = tokio::time::timeout(Duration::from_secs(35), async {
        let connection = ssh::authenticate(profile, password).await?;
        let mut channel = connection.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        ssh::check_reply(&mut channel, "SFTP subsystem").await?;
        let sftp = SftpSession::new(channel.into_stream()).await?;
        Ok::<_, anyhow::Error>((connection, sftp))
    })
    .await
    .context("Opening SFTP connection timed out")??;
    Ok((_connection, sftp))
}

pub async fn list(profile: &Ssh, path: &str) -> Result<(String, Vec<(String, bool)>)> {
    let (_connection, sftp) = connect(profile, None).await?;
    let directory = sftp.canonicalize(path).await?;
    let mut entries = Vec::new();
    for entry in sftp.read_dir(&directory).await? {
        let name = entry.file_name();
        if name == "." || name == ".." || name.contains('/') {
            continue;
        }
        let kind = entry.file_type();
        if kind.is_dir() || kind.is_file() {
            entries.push((name, kind.is_dir()));
        }
    }
    entries.sort_by_key(|(name, directory)| (!directory, name.to_lowercase()));
    Ok((directory, entries))
}

async fn upload(
    sftp: &SftpSession,
    local: &Path,
    remote: &str,
    retrying: bool,
    progress: &mut impl FnMut(String),
) -> Result<()> {
    let mut source = File::open(local)
        .await
        .with_context(|| format!("Opening {}", local.display()))?;
    let initial = source.metadata().await?;
    ensure!(initial.is_file(), "Local source must be a regular file");
    let total = initial.len();
    let partial = format!("{remote}.port-studio.part");
    if sftp.try_exists(remote).await? {
        // A dropped connection can hide a successful rename response.
        ensure!(
            retrying && !sftp.try_exists(&partial).await?,
            "Remote destination already exists"
        );
        let mut destination = sftp.open(remote).await?;
        ensure!(
            destination.metadata().await?.size == Some(total),
            "Remote destination already exists"
        );
        verify_prefix(&mut source, &mut destination, total).await?;
        progress(format!("Complete · {total} bytes"));
        return Ok(());
    }
    let mut destination = match sftp.symlink_metadata(&partial).await {
        Ok(attrs) => {
            ensure!(attrs.is_regular(), "Remote partial must be a regular file");
            sftp.open_with_flags(&partial, OpenFlags::READ | OpenFlags::WRITE)
                .await?
        }
        Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => {
            sftp.open_with_flags(
                &partial,
                OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE,
            )
            .await?
        }
        Err(e) => return Err(e.into()),
    };
    let offset = destination
        .metadata()
        .await?
        .size
        .context("Server omitted partial file size")?;
    ensure!(offset <= total, "Remote partial is larger than the source");
    verify_prefix(&mut source, &mut destination, offset).await?;
    copy_remaining(&mut source, &mut destination, offset, total, progress).await?;
    let current = source.metadata().await?;
    ensure!(
        current.len() == total && current.modified()? == initial.modified()?,
        "Local source changed during upload"
    );
    ensure!(
        destination.metadata().await?.size == Some(total),
        "Remote partial size changed"
    );
    destination.sync_all().await?;
    destination.close().await?;
    ensure!(
        !sftp.try_exists(remote).await?,
        "Remote destination already exists"
    );
    sftp.rename(partial, remote).await?;
    progress(format!("Complete · {total} bytes"));
    Ok(())
}

async fn download(
    sftp: &SftpSession,
    local: &Path,
    remote: &str,
    progress: &mut impl FnMut(String),
) -> Result<()> {
    ensure!(
        !fs::try_exists(local).await?,
        "Local destination already exists"
    );
    let mut source = sftp.open(remote).await?;
    let initial = source.metadata().await?;
    let total = initial.size.context("Server omitted source file size")?;
    ensure!(initial.is_regular(), "Remote source must be a regular file");
    let mut name = local.as_os_str().to_owned();
    name.push(".port-studio.part");
    let partial = std::path::PathBuf::from(name);
    match fs::symlink_metadata(&partial).await {
        Ok(attrs) => ensure!(attrs.is_file(), "Local partial must be a regular file"),
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let mut destination = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&partial)
        .await?;
    let offset = destination.metadata().await?.len();
    ensure!(offset <= total, "Local partial is larger than the source");
    verify_prefix(&mut source, &mut destination, offset).await?;
    copy_remaining(&mut source, &mut destination, offset, total, progress).await?;
    let current = source.metadata().await?;
    ensure!(
        current.size == Some(total) && current.mtime == initial.mtime,
        "Remote source changed during download"
    );
    destination.sync_all().await?;
    drop(destination);
    fs::hard_link(&partial, local)
        .await
        .context("Publishing download without overwriting the destination")?;
    if let Err(error) = fs::remove_file(&partial).await {
        tracing::warn!(%error, "Could not remove completed partial file");
    }
    progress(format!("Complete · {total} bytes"));
    Ok(())
}

async fn verify_prefix(
    source: &mut (impl AsyncRead + Unpin),
    partial: &mut (impl AsyncRead + Unpin),
    mut length: u64,
) -> Result<()> {
    let mut expected = [0; 32768];
    let mut existing = [0; 32768];
    while length > 0 {
        let count = length.min(expected.len() as u64) as usize;
        source.read_exact(&mut expected[..count]).await?;
        partial.read_exact(&mut existing[..count]).await?;
        ensure!(
            expected[..count] == existing[..count],
            "Partial file differs from source; move it aside before retrying"
        );
        length -= count as u64;
    }
    Ok(())
}

async fn copy_remaining(
    source: &mut (impl AsyncRead + Unpin),
    destination: &mut (impl AsyncWrite + AsyncSeekExt + Unpin),
    mut offset: u64,
    total: u64,
    progress: &mut impl FnMut(String),
) -> Result<()> {
    destination.seek(SeekFrom::Start(offset)).await?;
    progress(format!(
        "{} at {offset}/{total} bytes",
        if offset > 0 { "Resuming" } else { "Starting" }
    ));
    let mut buffer = [0; 32768];
    let mut last = Instant::now();
    while offset < total {
        let count = (total - offset).min(buffer.len() as u64) as usize;
        source.read_exact(&mut buffer[..count]).await?;
        destination.write_all(&buffer[..count]).await?;
        offset += count as u64;
        if last.elapsed() >= Duration::from_millis(500) {
            progress(format!("{offset}/{total} bytes"));
            last = Instant::now();
        }
    }
    destination.flush().await?;
    Ok(())
}

fn retryable(error: &anyhow::Error) -> bool {
    if error
        .downcast_ref::<tokio::time::error::Elapsed>()
        .is_some()
    {
        return true;
    }
    if let Some(e) = error.downcast_ref::<SftpError>() {
        return retryable_sftp(e);
    }
    if let Some(e) = error.downcast_ref::<std::io::Error>() {
        if let Some(sftp) = e.get_ref().and_then(|e| e.downcast_ref::<SftpError>()) {
            return retryable_sftp(sftp);
        }
        return matches!(
            e.kind(),
            ErrorKind::BrokenPipe
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::NotConnected
                | ErrorKind::TimedOut
                | ErrorKind::ConnectionRefused
        );
    }
    error.downcast_ref::<russh::Error>().is_some_and(|e| {
        matches!(
            e,
            russh::Error::Disconnect | russh::Error::SendError | russh::Error::KeepaliveTimeout
        )
    })
}
fn retryable_sftp(error: &SftpError) -> bool {
    match error {
        SftpError::Timeout | SftpError::IO(_) => true,
        SftpError::UnexpectedBehavior(message) => {
            message.starts_with("SendError:") || message.starts_with("RecvError:")
        }
        SftpError::Status(status) => matches!(
            status.status_code,
            StatusCode::NoConnection | StatusCode::ConnectionLost
        ),
        _ => false,
    }
}
