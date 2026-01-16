use anyhow::{anyhow, Result};
use pueue_lib::message::*;
use pueue_lib::settings::Settings;
use pueue_lib::state::State;
use pueue_lib::Client;
use pueue_lib::network::socket::ConnectionSettings;
use pueue_lib::secret::read_shared_secret;
use pueue_lib::tls::load_certificate;

pub trait PueueClientOps {
    async fn get_state(&mut self) -> Result<State>;
    async fn start_tasks(&mut self, ids: Vec<usize>) -> Result<()>;
    async fn restart_tasks(&mut self, tasks: Vec<TaskToRestart>) -> Result<()>;
    async fn pause_tasks(&mut self, ids: Vec<usize>) -> Result<()>;
    async fn kill_tasks(&mut self, ids: Vec<usize>) -> Result<()>;
    async fn remove_tasks(&mut self, ids: Vec<usize>) -> Result<()>;
    /// Start streaming logs for a task. Returns the initial log content.
    /// Use `receive_stream_chunk` to get subsequent chunks.
    async fn start_log_stream(&mut self, id: usize, lines: Option<usize>) -> Result<String>;
    /// Receive the next chunk of streamed logs. Returns None if stream closed.
    async fn receive_stream_chunk(&mut self) -> Result<Option<String>>;
    /// Reconnect to the pueue daemon.
    async fn reconnect(&mut self) -> Result<()>;
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

    async fn restart_tasks(&mut self, tasks: Vec<TaskToRestart>) -> Result<()> {
        self.client.send_request(Request::Restart(RestartRequest {
            tasks,
            start_immediately: true,
            stashed: false,
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

    async fn start_log_stream(&mut self, id: usize, lines: Option<usize>) -> Result<String> {
        self.client.send_request(Request::Stream(StreamRequest {
            tasks: TaskSelection::TaskIds(vec![id]),
            lines,
        })).await.map_err(|e| anyhow!("Failed to send stream request: {:?}", e))?;

        // First response contains the initial log content
        let response = self.client.receive_response().await.map_err(|e| anyhow!("Failed to receive stream response: {:?}", e))?;

        match response {
            Response::Stream(stream_response) => {
                Ok(stream_response.logs.get(&id).cloned().unwrap_or_default())
            }
            Response::Failure(msg) => Err(anyhow!("Stream request failed: {}", msg)),
            _ => Err(anyhow!("Unexpected response from pueue daemon: {:?}", response)),
        }
    }

    async fn receive_stream_chunk(&mut self) -> Result<Option<String>> {
        let response = self.client.receive_response().await.map_err(|e| anyhow!("Failed to receive stream chunk: {:?}", e))?;

        match response {
            Response::Stream(stream_response) => {
                // Concatenate all task logs (typically just one)
                let combined: String = stream_response.logs.values().cloned().collect();
                Ok(Some(combined))
            }
            Response::Close => Ok(None),
            Response::Failure(msg) => Err(anyhow!("Stream failed: {}", msg)),
            _ => Err(anyhow!("Unexpected response during streaming: {:?}", response)),
        }
    }

    async fn reconnect(&mut self) -> Result<()> {
        let new_self = Self::new().await?;
        self.client = new_self.client;
        Ok(())
    }
}
