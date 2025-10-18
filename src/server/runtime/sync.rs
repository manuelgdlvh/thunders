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
        hooks::{Diff, DiffNotification, Event, GameHooks},
        protocol::SessionManager,
        runtime::{GameHandle, GameRuntime},
    },
};

pub struct SyncRuntime {
    type_: &'static str,
    id: String,
    action_timeout: Duration,
    tick: Duration,
    session_manager: Arc<SessionManager>,
    players_cxt: HashMap<u64, Arc<PlayerContext>>,
}

pub struct Settings {
    pub max_action_await_millis: u64,
    pub tick_interval_millis: u64,
}

impl SyncRuntime {
    fn notify<H: GameHooks, S: Schema>(&self, diff: Diff<H::Delta>)
    where
        H::Delta: Serialize<S>,
    {
        match diff {
            Diff::All { delta } => {
                let diff = DiffNotification::new(self.type_, self.id.as_str(), delta.serialize());
                self.session_manager
                    .send_all(self.players_cxt.keys(), &diff);
            }
            Diff::Target { ids, delta } => {
                let diff = DiffNotification::new(self.type_, self.id.as_str(), delta.serialize());
                self.session_manager.send_all(ids.iter(), &diff);
            }
        }
    }
}

impl<H, S> GameRuntime<H, S> for SyncRuntime
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
        settings: &Self::Settings,
        session_manager: Arc<SessionManager>,
    ) -> Self {
        Self {
            id,
            type_,
            action_timeout: Duration::from_millis(settings.max_action_await_millis),
            tick: Duration::from_millis(settings.tick_interval_millis),
            session_manager,
            players_cxt: Default::default(),
        }
    }

    fn start(mut self, options: <H as GameHooks>::Options) -> Self::Handle {
        let mut hooks = H::build(options);
        let (actions_tx, actions_rx) = mpsc::channel::<(u64, Event<H>)>();
        let r_handle = thread::spawn(move || {
            let mut actions_buffer = Vec::new();
            let mut now;
            let mut tick;

            loop {
                let (is_finished, diff_opt) = hooks.finish();

                if is_finished {
                    if let Some(diff) = diff_opt {
                        self.notify::<H, S>(diff);
                    }

                    let diff = DiffNotification::finish(self.type_, self.id.as_str());
                    self.session_manager
                        .send_all(self.players_cxt.keys(), &diff);
                    break;
                }
                if let Ok(event) = actions_rx.recv_timeout(self.action_timeout) {
                    match event.1 {
                        Event::Action(action) => {
                            actions_buffer.push((event.0, action));
                            now = Instant::now();
                            tick = self.tick;
                        }

                        Event::Leave(id) => {
                            if let Some(player_context) = self.players_cxt.remove(&id)
                                && let Some(diff) = hooks.leave(player_context.as_ref())
                            {
                                self.notify::<H, S>(diff);
                            }

                            continue;
                        }

                        Event::Join(cxt) => {
                            self.players_cxt.insert(cxt.id(), Arc::clone(&cxt));
                            if let Some(diffs) = hooks.join(cxt.as_ref()) {
                                for diff in diffs {
                                    self.notify::<H, S>(diff);
                                }
                            }
                            continue;
                        }
                    }
                } else {
                    continue;
                }

                while let Ok(event) = actions_rx.recv_timeout(tick) {
                    match event.1 {
                        Event::Action(action) => {
                            actions_buffer.push((event.0, action));
                        }
                        Event::Leave(id) => {
                            if let Some(player_context) = self.players_cxt.remove(&id)
                                && let Some(diff) = hooks.leave(player_context.as_ref())
                            {
                                self.notify::<H, S>(diff);
                            }
                        }

                        Event::Join(cxt) => {
                            self.players_cxt.insert(cxt.id(), Arc::clone(&cxt));
                            if let Some(diffs) = hooks.join(cxt.as_ref()) {
                                for diff in diffs {
                                    self.notify::<H, S>(diff);
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

                for diff in hooks.diff(&self.players_cxt, actions_buffer.as_slice()) {
                    self.notify::<H, S>(diff);
                }

                hooks.update(mem::take(&mut actions_buffer));
            }
        });

        SyncGameHandle {
            actions: actions_tx,
            _r_handle: r_handle,
        }
    }
}

pub struct SyncGameHandle<H>
where
    H: GameHooks,
{
    actions: mpsc::Sender<(u64, Event<H>)>,
    _r_handle: JoinHandle<()>,
}

impl<H> GameHandle<H> for SyncGameHandle<H>
where
    H: GameHooks,
{
    fn event(&self, p_id: u64, event: Event<H>) {
        match &event {
            Event::Action(action) => {
                log::trace!("SERVER received action request. Action: {action:?} ");
            }
            Event::Join(cxt) => {
                log::trace!("SERVER received join request. PlayerContext: {cxt:?} ");
            }

            Event::Leave(id) => {
                log::trace!("SERVER received leave request. PlayerId: {id} ");
            }
        }

        if self.actions.send((p_id, event)).is_err() {
            log::warn!("Game runtime stopped, skipping action.");
        }
    }
}
