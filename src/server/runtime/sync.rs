use std::{
    collections::HashMap,
    mem,
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    api::{
        message::OutputMessage,
        schema::{Deserialize, Schema, Serialize},
    },
    server::{
        context::PlayerContext,
        hooks::{Diff, DiffNotification, GameHooks},
        protocol::SessionManager,
        runtime::{GameHandle, GameRuntime, RuntimeAction},
    },
};

pub struct SyncRuntime<H>
where
    H: GameHooks,
{
    type_: &'static str,
    id: String,
    hooks: H,
    tick_no_action: Duration,
    tick: Duration,
    session_manager: Arc<SessionManager>,
    players_cxts: HashMap<u64, Arc<PlayerContext>>,
}

pub struct Settings {
    pub tick_no_action_millis: u64,
    pub tick_millis: u64,
}

impl<H> SyncRuntime<H>
where
    H: GameHooks,
{
    fn notify<S: Schema>(&self, diff: Diff<H::Delta>)
    where
        H::Delta: Serialize<S>,
    {
        match diff {
            Diff::All { delta } => {
                let diff = DiffNotification::new(self.type_, self.id.as_str(), delta.serialize());
                self.session_manager
                    .send_all(self.players_cxts.keys(), &diff);
            }
            Diff::TargetUnique { id, delta } => {
                let diff = DiffNotification::new(self.type_, self.id.as_str(), delta.serialize());
                self.session_manager.send(id, &diff);
            }
            Diff::TargetList { ids, delta } => {
                let diff = DiffNotification::new(self.type_, self.id.as_str(), delta.serialize());
                self.session_manager.send_all(ids.iter(), &diff);
            }
        }
    }
}

impl<H, S> GameRuntime<H, S> for SyncRuntime<H>
where
    H: GameHooks,
    S: Schema,
    H::Delta: Serialize<S>,
    H::Options: for<'a> Deserialize<'a, S>,
    H::Action: for<'a> Deserialize<'a, S>,
    for<'a> OutputMessage<'a>: Serialize<S>,
{
    type Handle = SyncGameHandle<H>;
    type Settings = Settings;

    fn build(
        type_: &'static str,
        id: String,
        hooks: H,
        settings: &Self::Settings,
        session_manager: Arc<SessionManager>,
    ) -> Self {
        Self {
            id,
            type_,
            hooks,
            tick_no_action: Duration::from_millis(settings.tick_no_action_millis),
            tick: Duration::from_millis(settings.tick_millis),
            session_manager,
            players_cxts: Default::default(),
        }
    }

    fn start(mut self) -> Self::Handle {
        let (action_tx, action_rx) = mpsc::channel::<(u64, RuntimeAction<H>)>();
        let r_handle = thread::spawn(move || {
            let mut actions_buffer = Vec::new();
            let mut now;
            let mut tick;

            loop {
                let (is_finished, diff_opt) = self.hooks.is_finished();
                if is_finished {
                    if let Some(diff) = diff_opt {
                        self.notify::<S>(diff);
                    }

                    let diff = DiffNotification::finish(self.type_, self.id.as_str());
                    self.session_manager
                        .send_all(self.players_cxts.keys(), &diff);
                    break;
                }
                if let Ok(event) = action_rx.recv_timeout(self.tick_no_action) {
                    match event.1 {
                        RuntimeAction::Action(action) => {
                            actions_buffer.push((event.0, action));
                            now = Instant::now();
                            tick = self.tick;
                        }

                        RuntimeAction::Leave(id) => {
                            if let Some(player_context) = self.players_cxts.remove(&id)
                                && let Some(diff) = self.hooks.on_leave(player_context.as_ref())
                            {
                                self.notify::<S>(diff);
                            }

                            continue;
                        }

                        RuntimeAction::Join(cxt) => {
                            self.players_cxts.insert(cxt.id(), Arc::clone(&cxt));
                            if let Some(diffs) = self.hooks.on_join(cxt.as_ref()) {
                                for diff in diffs {
                                    self.notify::<S>(diff);
                                }
                            }
                            continue;
                        }
                    }
                } else {
                    if let Some(diffs) = self.hooks.on_tick(&self.players_cxts, vec![]) {
                        for diff in diffs {
                            self.notify::<S>(diff);
                        }
                    }
                    continue;
                }

                while let Ok(event) = action_rx.recv_timeout(tick) {
                    match event.1 {
                        RuntimeAction::Action(action) => {
                            actions_buffer.push((event.0, action));
                        }
                        RuntimeAction::Leave(id) => {
                            if let Some(player_context) = self.players_cxts.remove(&id)
                                && let Some(diff) = self.hooks.on_leave(player_context.as_ref())
                            {
                                self.notify::<S>(diff);
                            }
                        }

                        RuntimeAction::Join(cxt) => {
                            self.players_cxts.insert(cxt.id(), Arc::clone(&cxt));
                            if let Some(diffs) = self.hooks.on_join(cxt.as_ref()) {
                                for diff in diffs {
                                    self.notify::<S>(diff);
                                }
                            }
                        }
                    }

                    if let Some(new_tick) = tick.checked_sub(now.elapsed()) {
                        tick = new_tick;
                    } else {
                        break;
                    }
                }

                if let Some(diffs) = self
                    .hooks
                    .on_tick(&self.players_cxts, mem::take(&mut actions_buffer))
                {
                    for diff in diffs {
                        self.notify::<S>(diff);
                    }
                }
            }
        });

        SyncGameHandle {
            action_tx,
            _r_handle: r_handle,
        }
    }
}

pub struct SyncGameHandle<H>
where
    H: GameHooks,
{
    action_tx: mpsc::Sender<(u64, RuntimeAction<H>)>,
    _r_handle: JoinHandle<()>,
}

impl<H> GameHandle<H> for SyncGameHandle<H>
where
    H: GameHooks,
{
    fn send(&self, p_id: u64, r_action: RuntimeAction<H>) {
        match &r_action {
            RuntimeAction::Action(action) => {
                log::trace!("SERVER received action request. Action: {action:?} ");
            }
            RuntimeAction::Join(cxt) => {
                log::trace!("SERVER received join request. PlayerContext: {cxt:?} ");
            }

            RuntimeAction::Leave(id) => {
                log::trace!("SERVER received leave request. PlayerId: {id} ");
            }
        }

        if self.action_tx.send((p_id, r_action)).is_err() {
            log::warn!("Game runtime stopped, skipping action.");
        }
    }
}
