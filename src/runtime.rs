use std::{
    collections::HashMap,
    mem,
    sync::{Arc, RwLock, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    hooks::GameHooks,
    protocol::SessionManager,
    schema::{DeSerialize, Schema},
};

pub trait GameRuntime<H, S>
where
    S: Schema,
    H: GameHooks,
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    type Handle: GameHandle<H>;

    fn start(options: H::Options, session_manager: Arc<SessionManager>) -> Self::Handle;
}

pub struct SyncGameRuntime {}

impl<H, S> GameRuntime<H, S> for SyncGameRuntime
where
    H: GameHooks,
    S: Schema,
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    type Handle = SyncGameHandle<H>;

    fn start(
        options: <H as GameHooks>::Options,
        session_manager: Arc<SessionManager>,
    ) -> Self::Handle {
        let mut hooks = H::build(options);
        let (actions_tx, actions_rx) = mpsc::channel::<H::Action>();
        let r_handle = thread::spawn(move || {
            let mut actions_buffer = Vec::new();
            let mut now = Instant::now();
            let tick = Duration::from_millis(1000);
            loop {
                if let Ok(action) = actions_rx.recv_timeout(tick) {
                    actions_buffer.push(action);
                }

                if now.elapsed() >= tick {
                    now = Instant::now();
                    if !actions_buffer.is_empty() {
                        let delta = hooks.diff(actions_buffer.as_slice());
                        hooks.update(mem::take(&mut actions_buffer));
                        session_manager.send(1, delta.serialize());
                    }
                }

                if hooks.is_finished() {
                    break;
                }
            }
        });

        SyncGameHandle {
            _actions: actions_tx,
            _r_handle: r_handle,
        }
    }
}

// Add join and left player hooks
pub trait GameHandle<H>: Send + Sync
where
    H: GameHooks,
{
    fn action(&self, action: H::Action);
}

pub struct SyncGameHandle<H>
where
    H: GameHooks,
{
    _actions: mpsc::Sender<H::Action>,
    _r_handle: JoinHandle<()>,
}

impl<H> GameHandle<H> for SyncGameHandle<H>
where
    H: GameHooks,
{
    fn action(&self, action: <H as GameHooks>::Action) {
        self._actions.send(action).expect("");
    }
}

// Default async configurable and not with traits

pub struct GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    handlers: RwLock<HashMap<String, R::Handle>>,
    session_manager: Arc<SessionManager>,
}

impl<R, H, S> GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            session_manager,
        }
    }

    pub fn register(&self, id: String, options: H::Options) {
        let runtime = R::start(options, Arc::clone(&self.session_manager));
        if let Ok(mut handlers) = self.handlers.write() {
            handlers.insert(id, runtime);
        }
    }

    pub fn action(&self, id: String, action: H::Action) {
        if let Ok(handlers) = self.handlers.read() {
            if let Some(handler) = handlers.get(id.as_str()) {
                handler.action(action);
            }
        }
    }
}

pub trait GameRuntimeAnyHandle: Send + Sync {
    fn register(&self, id: String, options: Option<Vec<u8>>);
    fn action(&self, id: String, action: Vec<u8>);
}

impl<R, H, S> GameRuntimeAnyHandle for GameRuntimeHandle<R, H, S>
where
    R: GameRuntime<H, S>,
    H: GameHooks,
    S: Schema,
    H::Delta: DeSerialize<S>,
    H::Options: DeSerialize<S>,
    H::Action: DeSerialize<S>,
{
    fn register(&self, id: String, options: Option<Vec<u8>>) {
        if let Some(options) = options {
            let result = <H::Options as DeSerialize<S>>::deserialize(options).unwrap();
            self.register(id, result);
        } else {
            self.register(id, H::Options::default());
        }
    }

    fn action(&self, id: String, action: Vec<u8>) {
        let action = <H::Action as DeSerialize<S>>::deserialize(action).unwrap();
        self.action(id, action);
    }
}
