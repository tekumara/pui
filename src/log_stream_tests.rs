use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::{StreamExt, poll};
use pueue_lib::{message::TaskToRestart, state::State};
use ratatui::{Terminal, backend::TestBackend};
use tokio::sync::{Mutex, mpsc};

use crate::{App, AppMode, config::Config, pueue_client::PueueClientOps, tests::MockPueueClient};

#[derive(Clone, Copy)]
enum Mode {
    ConnectPending,
    RequestPending,
    Read,
    ConnectError,
    RequestError,
}

struct Control {
    mode: Mode,
    replies: Mutex<mpsc::UnboundedReceiver<Result<Option<String>>>>,
    connections: AtomicUsize,
    requests: AtomicUsize,
    reads: AtomicUsize,
    cancelled_reads: AtomicUsize,
    closed_connections: AtomicUsize,
}

struct Client {
    state: State,
    control: Arc<Control>,
    viewing: bool,
}

impl Drop for Client {
    fn drop(&mut self) {
        if self.viewing {
            self.control
                .closed_connections
                .fetch_add(1, Ordering::SeqCst);
        }
    }
}

struct ReadGuard {
    control: Arc<Control>,
    completed: bool,
}

impl Drop for ReadGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.control.cancelled_reads.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl PueueClientOps for Client {
    fn new(&self) -> impl std::future::Future<Output = Result<Self>> + 'static {
        let state = self.state.clone();
        let control = self.control.clone();
        async move {
            control.connections.fetch_add(1, Ordering::SeqCst);
            match control.mode {
                Mode::ConnectPending => std::future::pending::<()>().await,
                Mode::ConnectError => return Err(anyhow!("connection failed")),
                _ => {}
            }
            Ok(Self {
                state,
                control,
                viewing: true,
            })
        }
    }

    async fn get_state(&mut self) -> Result<State> {
        Ok(self.state.clone())
    }
    async fn start_tasks(&mut self, _: Vec<usize>) -> Result<()> {
        panic!("view must not start tasks")
    }
    async fn restart_tasks(&mut self, _: Vec<TaskToRestart>) -> Result<()> {
        panic!("view must not restart tasks")
    }
    async fn enqueue_tasks(&mut self, _: Vec<usize>) -> Result<()> {
        panic!("view must not enqueue tasks")
    }
    async fn pause_tasks(&mut self, _: Vec<usize>) -> Result<()> {
        panic!("view must not pause tasks")
    }
    async fn kill_tasks(&mut self, _: Vec<usize>) -> Result<()> {
        panic!("view must not kill tasks")
    }
    async fn remove_tasks(&mut self, _: Vec<usize>) -> Result<()> {
        panic!("view must not remove tasks")
    }
    async fn reconnect(&mut self) -> Result<()> {
        panic!("unexpected reconnect")
    }

    async fn start_log_stream(&mut self, id: usize, lines: Option<usize>) -> Result<()> {
        assert_eq!(id, 0);
        assert_eq!(lines, None);
        self.control.requests.fetch_add(1, Ordering::SeqCst);
        match self.control.mode {
            Mode::RequestPending => std::future::pending::<()>().await,
            Mode::RequestError => return Err(anyhow!("stream request failed")),
            _ => {}
        }
        Ok(())
    }

    async fn receive_stream_chunk(&mut self) -> Result<Option<String>> {
        self.control.reads.fetch_add(1, Ordering::SeqCst);
        let mut guard = ReadGuard {
            control: self.control.clone(),
            completed: false,
        };
        let reply = self.control.replies.lock().await.recv().await;
        guard.completed = true;
        reply.unwrap_or(Ok(None))
    }
}

type Fixture = (
    App<Client>,
    Terminal<TestBackend>,
    Arc<Control>,
    mpsc::UnboundedSender<Result<Option<String>>>,
);

async fn fixture(mode: Mode) -> Result<Fixture> {
    let state = MockPueueClient::new().get_state().await?;
    let (sender, receiver) = mpsc::unbounded_channel();
    let control = Arc::new(Control {
        mode,
        replies: Mutex::new(receiver),
        connections: AtomicUsize::new(0),
        requests: AtomicUsize::new(0),
        reads: AtomicUsize::new(0),
        cancelled_reads: AtomicUsize::new(0),
        closed_connections: AtomicUsize::new(0),
    });
    let mut app = App::new(
        Client {
            state,
            control: control.clone(),
            viewing: false,
        },
        Config::default(),
    );
    app.refresh_state().await?;
    let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
    terminal.draw(|frame| app.draw(frame))?;
    Ok((app, terminal, control, sender))
}

async fn key(
    app: &mut App<Client>,
    terminal: &mut Terminal<TestBackend>,
    code: KeyCode,
) -> Result<()> {
    tokio::time::timeout(
        Duration::from_secs(1),
        app.on_key_event(KeyEvent::new(code, KeyModifiers::NONE), terminal),
    )
    .await
    .expect("key handling must not wait for the viewing connection")
}

#[tokio::test]
async fn escape_cancels_pending_connection_request_or_read_only() -> Result<()> {
    for mode in [Mode::ConnectPending, Mode::RequestPending, Mode::Read] {
        let (mut app, mut terminal, control, _sender) = fixture(mode).await?;
        key(&mut app, &mut terminal, KeyCode::Enter).await?;
        assert!(matches!(app.app_mode, AppMode::Log(_)));
        // Connection setup itself must be deferred until the event loop polls the stream.
        assert_eq!(control.connections.load(Ordering::SeqCst), 0);
        assert!(poll!(app.log_stream.as_mut().unwrap().0.next()).is_pending());
        tokio::time::timeout(Duration::from_secs(1), app.refresh_state()).await??;
        terminal.draw(|frame| app.draw(frame))?;
        key(&mut app, &mut terminal, KeyCode::Esc).await?;
        assert!(app.log_stream.is_none());
        assert!(matches!(app.app_mode, AppMode::Normal));
        if matches!(mode, Mode::Read) {
            assert_eq!(control.cancelled_reads.load(Ordering::SeqCst), 1);
        }
        if !matches!(mode, Mode::ConnectPending) {
            assert_eq!(control.closed_connections.load(Ordering::SeqCst), 1);
        }
        key(&mut app, &mut terminal, KeyCode::Char('q')).await?;
        assert!(!app.running);
    }
    Ok(())
}

#[tokio::test]
async fn pending_reads_survive_refreshes_then_deliver_first_and_later_output() -> Result<()> {
    let (mut app, mut terminal, control, sender) = fixture(Mode::Read).await?;
    key(&mut app, &mut terminal, KeyCode::Enter).await?;
    for _ in 0..4 {
        // Like a select! tick winning: drop next(), but keep the underlying protocol read.
        assert!(poll!(app.log_stream.as_mut().unwrap().0.next()).is_pending());
        app.refresh_state().await?;
        terminal.draw(|frame| app.draw(frame))?;
    }
    assert_eq!(control.connections.load(Ordering::SeqCst), 1);
    assert_eq!(control.requests.load(Ordering::SeqCst), 1);
    assert_eq!(control.reads.load(Ordering::SeqCst), 1);
    assert_eq!(control.cancelled_reads.load(Ordering::SeqCst), 0);
    for chunk in ["first output\n", "later output\n"] {
        sender.send(Ok(Some(chunk.to_string())))?;
        let update = tokio::time::timeout(
            Duration::from_secs(1),
            app.log_stream.as_mut().unwrap().0.next(),
        )
        .await?;
        app.on_log_update(update);
    }
    let AppMode::Log(log) = &app.app_mode else {
        panic!("log view closed unexpectedly")
    };
    assert_eq!(log.logs, "first output\nlater output\n");
    sender.send(Ok(None))?;
    let update = tokio::time::timeout(
        Duration::from_secs(1),
        app.log_stream.as_mut().unwrap().0.next(),
    )
    .await?;
    app.on_log_update(update);
    assert!(app.log_stream.is_none());
    assert!(app.error_modal.is_none());
    assert_eq!(control.closed_connections.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn empty_completed_stream_is_not_an_error() -> Result<()> {
    let (mut app, mut terminal, _control, sender) = fixture(Mode::Read).await?;
    sender.send(Ok(None))?;
    key(&mut app, &mut terminal, KeyCode::Enter).await?;
    let update = tokio::time::timeout(
        Duration::from_secs(1),
        app.log_stream.as_mut().unwrap().0.next(),
    )
    .await?;
    app.on_log_update(update);
    assert!(app.log_stream.is_none());
    assert!(app.error_modal.is_none());
    let AppMode::Log(log) = &app.app_mode else {
        panic!("expected an empty log view")
    };
    assert!(log.logs.is_empty());
    key(&mut app, &mut terminal, KeyCode::Esc).await?;
    Ok(())
}

#[tokio::test]
async fn stream_failures_are_reported_and_dismissible() -> Result<()> {
    for mode in [Mode::ConnectError, Mode::RequestError, Mode::Read] {
        let (mut app, mut terminal, _control, sender) = fixture(mode).await?;
        if matches!(mode, Mode::Read) {
            sender.send(Err(anyhow!("read failed")))?;
        }
        key(&mut app, &mut terminal, KeyCode::Enter).await?;
        let update = tokio::time::timeout(
            Duration::from_secs(1),
            app.log_stream.as_mut().unwrap().0.next(),
        )
        .await?;
        app.on_log_update(update);
        assert!(app.log_stream.is_none());
        assert!(matches!(app.app_mode, AppMode::Normal));
        assert!(app.error_modal.as_ref().unwrap().contains("failed"));
        key(&mut app, &mut terminal, KeyCode::Esc).await?;
        assert!(app.error_modal.is_none());
    }
    Ok(())
}
