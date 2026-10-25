use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use nym_sdk::mixnet::{
    IncludedSurbs, MixnetClientBuilder, MixnetMessageSender, Recipient, StoragePaths,
};
use nysiris_core::error::{CoreError, Result};
use nysiris_core::transport::Transport;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};

use crate::{InboundMessage, READY_TIMEOUT};

const SEND_TIMEOUT: Duration = Duration::from_secs(120);
const REPLY_SURBS: u32 = 3;

struct Job {
    recipient: String,
    payload: Vec<u8>,
    ack: mpsc::SyncSender<std::result::Result<(), String>>,
}

/// The real Nym transport: a background client thread plus a job channel.
///
/// Status is drawn from the local Nym client, so addresses are handed out
/// through [`NymTransport::address`] and shared in invites.
pub struct NymTransport {
    address: String,
    jobs: tokio_mpsc::UnboundedSender<Job>,
    inbound: Mutex<mpsc::Receiver<InboundMessage>>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}

impl NymTransport {
    /// Start a Nym client (or reuse the stored one) under `data_dir`.
    ///
    /// Blocks up to the ready timeout while the client connects. `gateway`
    /// overrides the network default when set.
    pub fn connect(data_dir: impl AsRef<Path>, gateway: Option<String>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        eprintln!(
            "ncc-daemon: connecting Nym client (storage={})",
            data_dir.display()
        );
        let (job_tx, job_rx) = tokio_mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let ready_for_error = ready_tx.clone();

        let handle = thread::Builder::new()
            .name("ncc-nym-transport".into())
            .spawn(move || run(job_rx, inbound_tx, ready_tx, shutdown_rx, data_dir, gateway))
            .map_err(|e| CoreError::Transport(format!("cannot start Nym thread: {e}")))?;

        match ready_rx.recv_timeout(READY_TIMEOUT) {
            Ok(Ok(address)) => Ok(Self {
                address,
                jobs: job_tx,
                inbound: Mutex::new(inbound_rx),
                shutdown: Mutex::new(Some(shutdown_tx)),
                thread: Mutex::new(Some(handle)),
            }),
            Ok(Err(message)) => {
                let _ = ready_for_error.send(Err(message.clone()));
                let _ = thread::spawn(move || {
                    let _ = handle.join();
                });
                Err(CoreError::Transport(message))
            }
            Err(_) => {
                let _ = shutdown_tx.send(());
                let _ = thread::spawn(move || {
                    let _ = handle.join();
                });
                Err(CoreError::Transport(
                    "timed out waiting for the Nym client to connect".into(),
                ))
            }
        }
    }

    /// Always `true`: this build has the real adapter.
    pub fn is_available() -> bool {
        true
    }

    /// This client's Nym address, for handing out in invites.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Wait up to `timeout` for one inbound message.
    ///
    /// Returns `Ok(None)` on timeout, and an error if the client thread died.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<InboundMessage>> {
        let inbound = self
            .inbound
            .lock()
            .map_err(|_| CoreError::Transport("Nym inbound lock poisoned".into()))?;
        match inbound.recv_timeout(timeout) {
            Ok(message) => Ok(Some(message)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(CoreError::Transport("Nym inbound channel closed".into()))
            }
        }
    }

    /// Ask the client thread to stop. Also called from `Drop`.
    pub fn shutdown(&self) {
        if let Ok(mut slot) = self.shutdown.lock() {
            if let Some(sender) = slot.take() {
                let _ = sender.send(());
            }
        }
    }
}

impl Drop for NymTransport {
    fn drop(&mut self) {
        self.shutdown();
        if let Ok(mut slot) = self.thread.lock() {
            if let Some(handle) = slot.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Transport for NymTransport {
    fn name(&self) -> &'static str {
        "nym"
    }

    fn send(&self, recipient: &str, payload: Vec<u8>) -> Result<()> {
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        self.jobs
            .send(Job {
                recipient: recipient.to_string(),
                payload,
                ack: ack_tx,
            })
            .map_err(|_| CoreError::Transport("Nym transport is shutting down".into()))?;
        match ack_rx.recv_timeout(SEND_TIMEOUT) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(CoreError::Transport(message)),
            Err(_) => Err(CoreError::Transport(
                "timed out waiting for the Nym send to complete".into(),
            )),
        }
    }
}

fn run(
    mut jobs: tokio_mpsc::UnboundedReceiver<Job>,
    inbound: mpsc::Sender<InboundMessage>,
    ready: mpsc::Sender<std::result::Result<String, String>>,
    shutdown: oneshot::Receiver<()>,
    data_dir: PathBuf,
    gateway: Option<String>,
) {
    let ready_for_error = ready.clone();
    let result = (|| -> std::result::Result<(), String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("cannot build Tokio runtime: {e}"))?;
        runtime.block_on(async move {
            let paths = StoragePaths::new_from_dir(data_dir)
                .map_err(|e| format!("Nym storage init failed: {e}"))?;
            let mut builder = MixnetClientBuilder::new_with_default_storage(paths)
                .await
                .map_err(|e| format!("Nym client builder failed: {e}"))?;
            if let Some(gateway) = gateway {
                builder = builder.request_gateway(gateway);
            }
            let mut client = builder
                .build()
                .map_err(|e| format!("Nym client build failed: {e}"))?
                .connect_to_mixnet()
                .await
                .map_err(|e| format!("Nym connect failed: {e}"))?;

            let address = client.nym_address().to_string();
            eprintln!("ncc-daemon: Nym address {address}");
            let sender = client.split_sender();
            if ready.send(Ok(address)).is_err() {
                client.disconnect().await;
                return Ok(());
            }

            let send_task = tokio::spawn(async move {
                while let Some(job) = jobs.recv().await {
                    let result = match job.recipient.parse::<Recipient>() {
                        Ok(recipient) => sender
                            .send_message(recipient, job.payload, IncludedSurbs::new(REPLY_SURBS))
                            .await
                            .map_err(|e| e.to_string()),
                        Err(e) => Err(format!("bad Nym recipient: {e}")),
                    };
                    let _ = job.ack.send(result);
                }
            });

            let mut shutdown = shutdown;
            loop {
                tokio::select! {
                    _ = &mut shutdown => break,
                    batch = client.wait_for_messages() => {
                        match batch {
                            Some(messages) => {
                                for message in messages {
                                    let _ = inbound.send(InboundMessage {
                                        payload: message.message,
                                        sender_tag: message.sender_tag.map(|tag| tag.to_bytes()),
                                    });
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
            send_task.abort();
            client.disconnect().await;
            Ok(())
        })
    })();

    if let Err(message) = result {
        eprintln!("ncc-daemon: Nym transport failed: {message}");
        let _ = ready_for_error.send(Err(message));
    }
}
