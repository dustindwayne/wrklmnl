use super::config::Ssh;
use anyhow::{bail, ensure, Context, Result};
use russh::{
    client,
    keys::{agent::client::AgentClient, PrivateKeyWithHashAlg, PublicKeyOrCertificate},
    Channel, ChannelMsg,
};
use std::{sync::Arc, time::Duration};

pub struct Verifier(Ssh);

impl client::Handler for Verifier {
    type Error = anyhow::Error;
    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = key else {
            bail!("SSH host certificates are unsupported");
        };
        let host = &self.0.host;
        let port = self.0.port;
        let matched = match &self.0.known_hosts {
            Some(path) => russh::keys::check_known_hosts_path(host, port, key, path),
            None => russh::keys::check_known_hosts(host, port, key),
        };
        ensure!(
            matched.with_context(|| format!("Checking known_hosts for {host}:{port}"))?,
            "Host key for {host}:{port} is missing from known_hosts"
        );
        Ok(true)
    }
}

pub async fn authenticate(
    profile: &Ssh,
    password: Option<&str>,
) -> Result<client::Handle<Verifier>> {
    let config = client::Config {
        keepalive_interval: Some(Duration::from_secs(20)),
        keepalive_max: 3,
        ..Default::default()
    };
    let mut connection = client::connect(
        Arc::new(config),
        (profile.host.as_str(), profile.port),
        Verifier(profile.clone()),
    )
    .await
    .context("Opening SSH connection")?;
    let user = &profile.user;
    let authenticated = if let Some(path) = profile.identity.clone() {
        let passphrase = password.map(str::to_owned);
        let key = tokio::task::spawn_blocking(move || {
            russh::keys::load_secret_key(path, passphrase.as_deref())
        })
        .await??;
        let hash = connection.best_supported_rsa_hash().await?.flatten();
        connection
            .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
            .await?
            .success()
    } else if let Some(password) = password {
        connection
            .authenticate_password(user, password)
            .await?
            .success()
    } else {
        #[cfg(unix)]
        let mut agent = AgentClient::connect_env()
            .await
            .context("Connecting to SSH_AUTH_SOCK")?;
        #[cfg(windows)]
        let mut agent = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
            .await
            .context("Connecting to Windows OpenSSH agent")?;
        let hash = connection.best_supported_rsa_hash().await?.flatten();
        let mut accepted = false;
        for key in agent.request_identities().await? {
            if let russh::keys::agent::AgentIdentity::PublicKey { key, .. } = key {
                if connection
                    .authenticate_publickey_with(user, key, hash, &mut agent)
                    .await?
                    .success()
                {
                    accepted = true;
                    break;
                }
            }
        }
        accepted
    };
    ensure!(authenticated, "SSH authentication rejected");
    Ok(connection)
}

pub async fn connect(
    profile: &Ssh,
    password: Option<&str>,
    cols: u16,
    rows: u16,
) -> Result<(client::Handle<Verifier>, Channel<client::Msg>)> {
    let connection = authenticate(profile, password).await?;
    let mut channel = connection.channel_open_session().await?;
    channel
        .request_pty(true, "xterm", cols.into(), rows.into(), 0, 0, &[])
        .await?;
    check_reply(&mut channel, "PTY").await?;
    channel.request_shell(true).await?;
    check_reply(&mut channel, "shell").await?;
    Ok((connection, channel))
}

pub async fn check_reply(channel: &mut Channel<client::Msg>, request: &str) -> Result<()> {
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Success) => return Ok(()),
            Some(ChannelMsg::Failure) | Some(ChannelMsg::Close) | None => {
                bail!("SSH server rejected the {request} request")
            }
            _ => {}
        }
    }
}
