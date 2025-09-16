use std::{
    mem,
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    core::hooks::GameHooks,
    protocol::SessionManager,
    runtime::{GameHandle, GameRuntime},
    schema::{DeSerialize, Schema},
};

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
