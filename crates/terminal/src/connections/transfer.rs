use super::{config::Ssh, ssh};
use anyhow::{ensure, Context, Result};
use russh_sftp::{
    client::{error::Error as SftpError, SftpSession},
    protocol::{OpenFlags, StatusCode},
};
use std::{
    io::{ErrorKind, SeekFrom},
    path::{Path, PathBuf},
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

const PARTIAL_SUFFIX: &str = ".wrklmnl.part";
const LEGACY_PARTIAL_SUFFIX: &str = ".port-studio.part";

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
    let mut partial = format!("{remote}{PARTIAL_SUFFIX}");
    let mut partial_metadata = remote_partial_metadata(sftp, &partial).await?;
    if partial_metadata.is_none() {
        let legacy = format!("{remote}{LEGACY_PARTIAL_SUFFIX}");
        if let Some(metadata) = remote_partial_metadata(sftp, &legacy).await? {
            partial = legacy;
            partial_metadata = Some(metadata);
        }
    }
    if sftp.try_exists(remote).await? {
        // A dropped connection can hide a successful rename response.
        ensure!(
            retrying && partial_metadata.is_none(),
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
    let mut destination = match partial_metadata {
        Some(attrs) => {
            ensure!(attrs.is_regular(), "Remote partial must be a regular file");
            sftp.open_with_flags(&partial, OpenFlags::READ | OpenFlags::WRITE)
                .await?
        }
        None => {
            sftp.open_with_flags(
                &partial,
                OpenFlags::READ | OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE,
            )
            .await?
        }
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

async fn remote_partial_metadata(
    sftp: &SftpSession,
    path: &str,
) -> Result<Option<russh_sftp::protocol::FileAttributes>> {
    match sftp.symlink_metadata(path).await {
        Ok(metadata) => Ok(Some(metadata)),
        Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => Ok(None),
        Err(error) => Err(error.into()),
    }
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
    let (partial, mut destination) = open_local_partial(local).await?;
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

async fn open_local_partial(destination: &Path) -> Result<(PathBuf, File)> {
    let mut preferred = None;
    for suffix in [PARTIAL_SUFFIX, LEGACY_PARTIAL_SUFFIX] {
        let mut path = destination.as_os_str().to_owned();
        path.push(suffix);
        let path = PathBuf::from(path);
        match fs::symlink_metadata(&path).await {
            Ok(attrs) => {
                ensure!(attrs.is_file(), "Local partial must be a regular file");
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .await?;
                return Ok((path, file));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                preferred.get_or_insert(path);
            }
            Err(error) => return Err(error.into()),
        }
    }
    let path = preferred.context("Missing partial file path")?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .await?;
    Ok((path, file))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn partial_files_use_current_name_and_resume_legacy_downloads() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let destination = directory.path().join("download.txt");
        let (path, file) = open_local_partial(&destination).await?;
        assert_eq!(path.file_name().unwrap(), "download.txt.wrklmnl.part");
        drop(file);
        fs::remove_file(path).await?;

        let legacy = directory.path().join("download.txt.port-studio.part");
        fs::write(&legacy, b"existing prefix").await?;
        let (path, mut file) = open_local_partial(&destination).await?;
        assert_eq!(path, legacy);
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).await?;
        assert_eq!(bytes, b"existing prefix");
        Ok(())
    }

    #[tokio::test]
    async fn existing_current_partial_takes_precedence() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let destination = directory.path().join("download.txt");
        let current = directory.path().join("download.txt.wrklmnl.part");
        fs::write(&current, b"current prefix").await?;
        fs::write(
            directory.path().join("download.txt.port-studio.part"),
            b"legacy prefix",
        )
        .await?;
        let (path, file) = open_local_partial(&destination).await?;
        assert_eq!(path, current);
        assert_eq!(file.metadata().await?.len(), 14);
        Ok(())
    }
}
