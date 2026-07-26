//! Browser provider: o daemon roda como Native Messaging host, spawnado
//! pelo navegador via `connectNative` e conectado por stdin/stdout — sem
//! socket de rede. O canal fecha sozinho quando a extensão desconecta
//! (aba fechada, browser fechado etc.), e o processo deve encerrar.
//!
//! Além disso, se nenhum evento chegar por `IDLE_TIMEOUT`, emite um
//! `SessionEnded` sintético — cobre o caso de a aba continuar aberta sem
//! tocar nada (ex: página parada, sem `pause`/`ended` disparando).

#![forbid(unsafe_code)]

mod protocol;

use std::time::Duration;

use async_trait::async_trait;
use sonata_core::{Error, MediaEvent, MediaEventKind, MediaProvider, MediaSource, Result};
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::mpsc,
    task::JoinHandle,
};
use tracing::{debug, warn};

use protocol::WireMessage;

/// Limite oficial de mensagem do protocolo de Native Messaging do Chrome.
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// Tempo sem nenhum evento após o qual consideramos a sessão encerrada.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct BrowserProvider {
    events: mpsc::UnboundedReceiver<Result<WireMessage>>,
    reader: JoinHandle<()>,
    last_source: Option<MediaSource>,
    ended: bool,
}

impl BrowserProvider {
    /// Começa a ler o stdin do processo como um canal de Native Messaging.
    pub fn connect() -> Result<Self> {
        let (sender, events) = mpsc::unbounded_channel();
        let reader = tokio::spawn(read_loop(sender));

        debug!("browser provider connected (reading native messaging stdin)");

        Ok(Self {
            events,
            reader,
            last_source: None,
            ended: false,
        })
    }
}

#[async_trait]
impl MediaProvider for BrowserProvider {
    async fn next_event(&mut self) -> Result<MediaEvent> {
        tokio::select! {
            message = self.events.recv() => {
                let message = message.ok_or_else(|| {
                    Error::Provider("the native messaging reader stopped unexpectedly".into())
                })??;

                let event = message.into_media_event()?;
                self.last_source = Some(event.source.clone());
                self.ended = matches!(event.kind, MediaEventKind::SessionEnded);
                Ok(event)
            }
            _ = tokio::time::sleep(IDLE_TIMEOUT), if self.last_source.is_some() && !self.ended => {
                debug!("no browser events for {IDLE_TIMEOUT:?}, clearing presence");
                self.ended = true;
                Ok(MediaEvent::session_ended(
                    self.last_source.clone().expect("checked by the select guard"),
                ))
            }
        }
    }
}

impl Drop for BrowserProvider {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

/// Lê frames do Native Messaging em loop e os empurra num canal — mantido
/// numa task própria, nunca cancelada pelo `select!` de cima, pra nenhuma
/// leitura de frame ficar pela metade (o que corromperia o framing
/// permanentemente).
async fn read_loop(sender: mpsc::UnboundedSender<Result<WireMessage>>) {
    let mut stdin = BufReader::new(tokio::io::stdin());

    loop {
        let outcome = match read_frame(&mut stdin).await {
            Ok(Some(bytes)) => serde_json::from_slice::<WireMessage>(&bytes).map_err(|error| {
                Error::Provider(format!("invalid native messaging payload: {error}"))
            }),
            Ok(None) => {
                let _ = sender.send(Err(Error::Provider(
                    "native messaging channel closed".into(),
                )));
                return;
            }
            Err(error) => Err(error),
        };

        let is_err = outcome.is_err();
        if sender.send(outcome).is_err() {
            return; // o provider foi dropado, ninguém mais está ouvindo
        }
        if is_err {
            return;
        }
    }
}

/// Lê um frame no formato do Chrome/Firefox: 4 bytes de tamanho (native
/// endian) seguidos do payload JSON. Retorna `Ok(None)` em EOF limpo.
async fn read_frame(stdin: &mut BufReader<tokio::io::Stdin>) -> Result<Option<Vec<u8>>> {
    let mut length_buf = [0u8; 4];
    match stdin.read_exact(&mut length_buf).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => {
            return Err(Error::Provider(format!(
                "failed to read native messaging frame length: {error}"
            )))
        }
    }

    let length = u32::from_ne_bytes(length_buf) as usize;
    if length > MAX_MESSAGE_BYTES {
        warn!(length, "native messaging frame exceeds the expected limit");
        return Err(Error::Provider(format!(
            "native messaging frame too large: {length} bytes"
        )));
    }

    let mut payload = vec![0u8; length];
    stdin.read_exact(&mut payload).await.map_err(|error| {
        Error::Provider(format!("failed to read native messaging payload: {error}"))
    })?;

    Ok(Some(payload))
}