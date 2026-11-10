use super::session::Event;
use anyhow::{Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};

const IAC: u8 = 255;

#[derive(Default)]
enum State {
    #[default]
    Data,
    Command,
    Option(u8),
    Subnegotiation,
    Subcommand,
}

#[derive(Default)]
struct Decoder {
    state: State,
    after_cr: bool,
    echo: bool,
    suppress_ga: bool,
}

impl Decoder {
    fn decode(&mut self, chunk: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut data = Vec::new();
        let mut reply = Vec::new();
        for &byte in chunk {
            self.state = match std::mem::take(&mut self.state) {
                State::Data if byte == IAC => State::Command,
                State::Data => {
                    if !(self.after_cr && byte == 0) {
                        data.push(byte);
                    }
                    self.after_cr = byte == b'\r';
                    State::Data
                }
                State::Command => match byte {
                    IAC => {
                        data.push(IAC);
                        self.after_cr = false;
                        State::Data
                    }
                    250 => State::Subnegotiation,
                    251..=254 => State::Option(byte),
                    _ => State::Data,
                },
                State::Option(command) => {
                    let supported = match byte {
                        1 => Some(&mut self.echo),
                        3 => Some(&mut self.suppress_ga),
                        _ => None,
                    };
                    match (command, supported) {
                        (251, Some(enabled)) if !*enabled => {
                            *enabled = true;
                            reply.extend([IAC, 253, byte]);
                        }
                        (252, Some(enabled)) if *enabled => {
                            *enabled = false;
                            reply.extend([IAC, 254, byte]);
                        }
                        (251, None) => reply.extend([IAC, 254, byte]),
                        (253, _) => reply.extend([IAC, 252, byte]),
                        _ => {}
                    }
                    State::Data
                }
                State::Subnegotiation if byte == IAC => State::Subcommand,
                State::Subnegotiation => State::Subnegotiation,
                State::Subcommand if byte == 240 => State::Data,
                State::Subcommand => State::Subnegotiation,
            };
        }
        (data, reply)
    }
}

pub async fn run(
    host: &str,
    port: u16,
    mut input: mpsc::Receiver<Vec<u8>>,
    events: &mpsc::Sender<Event>,
) -> Result<()> {
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        TcpStream::connect((host, port)),
    )
    .await
    .context("Telnet connection timed out")??;
    if events.send(Event::Connected).await.is_err() {
        return Ok(());
    }
    let mut decoder = Decoder::default();
    let mut buffer = [0; 4096];
    let mut input_open = true;
    loop {
        tokio::select! {
            count = stream.read(&mut buffer) => {
                let count = count?;
                if count == 0 { return Ok(()); }
                let (data, reply) = decoder.decode(&buffer[..count]);
                if !reply.is_empty() { stream.write_all(&reply).await?; }
                if !data.is_empty() && events.send(Event::Rx(data)).await.is_err() { return Ok(()); }
            }
            message = input.recv(), if input_open => match message {
                Some(bytes) => {
                    let mut wire = Vec::new();
                    for byte in bytes {
                        match byte {
                            IAC => wire.extend([IAC, IAC]),
                            b'\r' => wire.extend(b"\r\n"),
                            _ => wire.push(byte),
                        }
                    }
                    stream.write_all(&wire).await?;
                }
                _ => { input_open = false; stream.shutdown().await?; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn negotiation_and_subnegotiation_survive_split_reads() {
        let mut decoder = super::Decoder::default();
        assert_eq!(decoder.decode(b"hi\r\xff"), (b"hi\r".to_vec(), vec![]));
        assert_eq!(
            decoder.decode(&[251, 1, 0, 255, 250, 24]),
            (vec![], vec![255, 253, 1])
        );
        assert_eq!(
            decoder.decode(&[42, 255, 240, 255, 255, b'x']),
            (vec![255, b'x'], vec![])
        );
    }
}
