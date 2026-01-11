use anyhow::{anyhow, Result};
use pueue_lib::message::*;
use pueue_lib::settings::Settings;
use pueue_lib::state::State;
use pueue_lib::Client;
use pueue_lib::network::socket::ConnectionSettings;
use pueue_lib::secret::read_shared_secret;
use pueue_lib::tls::load_certificate;

use snap::read::FrameDecoder;
use std::io::Read;

pub trait PueueClientOps {
    async fn get_state(&mut self) -> Result<State>;
    async fn start_tasks(&mut self, ids: Vec<usize>) -> Result<()>;
    async fn pause_tasks(&mut self, ids: Vec<usize>) -> Result<()>;
    async fn kill_tasks(&mut self, ids: Vec<usize>) -> Result<()>;
    async fn remove_tasks(&mut self, ids: Vec<usize>) -> Result<()>;
    async fn get_task_log(&mut self, id: usize) -> Result<Option<String>>;
}

#[derive(Debug)]
pub struct PueueClient {
    client: Client,
}

impl PueueClient {
    pub async fn new() -> Result<Self> {
        let (settings, _) = Settings::read(&None)?;
        let secret = read_shared_secret(&settings.shared.shared_secret_path())
            .map_err(|e| anyhow!("Failed to read shared secret: {:?}", e))?;

        let connection_settings = if settings.shared.use_unix_socket {
            ConnectionSettings::UnixSocket {
                path: settings.shared.unix_socket_path(),
            }
        } else {
            let cert = load_certificate(&settings.shared.daemon_cert())
                .map_err(|e| anyhow!("Failed to load daemon certificate: {:?}", e))?;
            ConnectionSettings::TlsTcpSocket {
                host: settings.shared.host.clone(),
                port: settings.shared.port.clone(),
                certificate: cert,
            }
        };

        let client = Client::new(connection_settings, &secret, false)
            .await
            .map_err(|e| anyhow!("{:?}", e))?;

        Ok(Self { client })
    }
}

impl PueueClientOps for PueueClient {
    async fn get_state(&mut self) -> Result<State> {
        self.client.send_request(Request::Status).await.map_err(|e| anyhow!("{:?}", e))?;
        let response = self.client.receive_response().await.map_err(|e| anyhow!("{:?}", e))?;

        if let Response::Status(state) = response {
            Ok(*state)
        } else {
            Err(anyhow!("Unexpected response from pueue daemon: {:?}", response))
        }
    }

    async fn start_tasks(&mut self, ids: Vec<usize>) -> Result<()> {
        self.client.send_request(Request::Start(StartRequest {
            tasks: TaskSelection::TaskIds(ids),
        })).await.map_err(|e| anyhow!("{:?}", e))?;
        let _ = self.client.receive_response().await.map_err(|e| anyhow!("{:?}", e))?;
        Ok(())
    }

    async fn pause_tasks(&mut self, ids: Vec<usize>) -> Result<()> {
        self.client.send_request(Request::Pause(PauseRequest {
            tasks: TaskSelection::TaskIds(ids),
            wait: false,
        })).await.map_err(|e| anyhow!("{:?}", e))?;
        let _ = self.client.receive_response().await.map_err(|e| anyhow!("{:?}", e))?;
        Ok(())
    }

    async fn kill_tasks(&mut self, ids: Vec<usize>) -> Result<()> {
        self.client.send_request(Request::Kill(KillRequest {
            tasks: TaskSelection::TaskIds(ids),
            signal: None,
        })).await.map_err(|e| anyhow!("{:?}", e))?;
        let _ = self.client.receive_response().await.map_err(|e| anyhow!("{:?}", e))?;
        Ok(())
    }

    async fn remove_tasks(&mut self, ids: Vec<usize>) -> Result<()> {
        self.client.send_request(Request::Remove(ids)).await.map_err(|e| anyhow!("{:?}", e))?;
        let _ = self.client.receive_response().await.map_err(|e| anyhow!("{:?}", e))?;
        Ok(())
    }

    async fn get_task_log(&mut self, id: usize) -> Result<Option<String>> {
        self.client.send_request(Request::Log(LogRequest {
            tasks: TaskSelection::TaskIds(vec![id]),
            send_logs: true,
            lines: None,
        })).await.map_err(|e| anyhow!("Failed to send log request: {:?}", e))?;

        let response = self.client.receive_response().await.map_err(|e| anyhow!("Failed to receive log response: {:?}", e))?;

        if let Response::Log(mut logs) = response {
            if let Some(task_log) = logs.remove(&id) {
                Ok(task_log.output.map(|bytes| self.decompress_log(bytes)))
            } else {
                Err(anyhow!("Task {} not found in log response", id))
            }
        } else {
            Err(anyhow!("Unexpected response from pueue daemon: {:?}", response))
        }
    }
}

impl PueueClient {
    fn decompress_log(&self, bytes: Vec<u8>) -> String {
        let mut decoder = FrameDecoder::new(&bytes[..]);
        let mut decompressed = Vec::new();

        match decoder.read_to_end(&mut decompressed) {
            Ok(_) => String::from_utf8_lossy(&decompressed).into_owned(),
            Err(_) => String::from_utf8_lossy(&bytes).into_owned(),
        }
    }
}
