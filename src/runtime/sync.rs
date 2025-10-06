use std::{
    collections::HashMap,
    mem,
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    core::{
        context::PlayerContext,
        hooks::{Diff, DiffNotification, Event, GameHooks},
    },
    protocol::SessionManager,
    runtime::{GameHandle, GameRuntime},
    schema::{Deserialize, Schema, Serialize},
};

pub struct SyncRuntime {
    type_: &'static str,
    id: String,
    action_timeout: Duration,
    tick: Duration,
}

pub struct Settings {
    pub max_action_await_millis: u64,
    pub tick_interval_millis: u64,
}

impl<H, S> GameRuntime<H, S> for SyncRuntime
where
    H: GameHooks,
    S: Schema,
    H::Delta: Serialize<S>,
    H::Options: Deserialize<S>,
    H::Action: Deserialize<S>,
    for<'a> DiffNotification<'a>: Serialize<S>,
{
    type Handle = SyncGameHandle<H>;
    type Settings = Settings;

    fn build(type_: &'static str, id: String, settings: &Self::Settings) -> Self {
        Self {
            id,
            type_,
            action_timeout: Duration::from_millis(settings.max_action_await_millis),
            tick: Duration::from_millis(settings.tick_interval_millis),
        }
    }

    fn start(
        self,
        options: <H as GameHooks>::Options,
        session_manager: Arc<SessionManager>,
    ) -> Self::Handle {
        let mut hooks = H::build(options);
        let (actions_tx, actions_rx) = mpsc::channel::<(u64, Event<H>)>();
        let r_handle = thread::spawn(move || {
            let mut actions_buffer = Vec::new();
            let mut now;
            let mut tick;

            let mut player_cxts: HashMap<u64, Arc<PlayerContext>> = HashMap::default();
            loop {
                let (is_finished, diff_opt) = hooks.finish();

                if is_finished {
                    if let Some(diff) = diff_opt {
                        match diff {
                            Diff::All { delta } => {
                                let delta = DiffNotification::new(
                                    self.type_,
                                    self.id.as_str(),
                                    delta.serialize(),
                                )
                                .serialize();
                                for p_id in player_cxts.keys() {
                                    session_manager.send(*p_id, delta.clone());
                                }
                            }
                            Diff::Target { ids, delta } => {
                                let delta = DiffNotification::new(
                                    self.type_,
                                    self.id.as_str(),
                                    delta.serialize(),
                                )
                                .serialize();
                                for &p_id in ids.iter() {
                                    session_manager.send(p_id, delta.clone());
                                }
                            }
                        }
                    }

                    let delta = DiffNotification::finish(self.type_, self.id.as_str()).serialize();
                    for p_id in player_cxts.keys() {
                        session_manager.send(*p_id, delta.clone());
                    }

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
                            if let Some(player_context) = player_cxts.remove(&id) {
                                if let Some(diff) = hooks.leave(player_context.as_ref()) {
                                    match diff {
                                        Diff::All { delta } => {
                                            let delta = DiffNotification::new(
                                                self.type_,
                                                self.id.as_str(),
                                                delta.serialize(),
                                            )
                                            .serialize();

                                            for p_id in player_cxts.keys() {
                                                session_manager.send(*p_id, delta.clone());
                                            }
                                        }
                                        Diff::Target { ids, delta } => {
                                            let delta = DiffNotification::new(
                                                self.type_,
                                                self.id.as_str(),
                                                delta.serialize(),
                                            )
                                            .serialize();

                                            for &p_id in ids.iter() {
                                                session_manager.send(p_id, delta.clone());
                                            }
                                        }
                                    }
                                }
                            }
                            continue;
                        }

                        Event::Join(cxt) => {
                            player_cxts.insert(cxt.id(), Arc::clone(&cxt));
                            if let Some(diffs) = hooks.join(cxt.as_ref()) {
                                for diff in diffs {
                                    match diff {
                                        Diff::All { delta } => {
                                            let delta = DiffNotification::new(
                                                self.type_,
                                                self.id.as_str(),
                                                delta.serialize(),
                                            )
                                            .serialize();

                                            for p_id in player_cxts.keys() {
                                                session_manager.send(*p_id, delta.clone());
                                            }
                                        }
                                        Diff::Target { ids, delta } => {
                                            let delta = DiffNotification::new(
                                                self.type_,
                                                self.id.as_str(),
                                                delta.serialize(),
                                            )
                                            .serialize();

                                            for &p_id in ids.iter() {
                                                session_manager.send(p_id, delta.clone());
                                            }
                                        }
                                    }
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
                            if let Some(player_context) = player_cxts.remove(&id) {
                                if let Some(diff) = hooks.leave(player_context.as_ref()) {
                                    match diff {
                                        Diff::All { delta } => {
                                            let delta = DiffNotification::new(
                                                self.type_,
                                                self.id.as_str(),
                                                delta.serialize(),
                                            )
                                            .serialize();

                                            for p_id in player_cxts.keys() {
                                                session_manager.send(*p_id, delta.clone());
                                            }
                                        }
                                        Diff::Target { ids, delta } => {
                                            let delta = DiffNotification::new(
                                                self.type_,
                                                self.id.as_str(),
                                                delta.serialize(),
                                            )
                                            .serialize();

                                            for &p_id in ids.iter() {
                                                session_manager.send(p_id, delta.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        Event::Join(cxt) => {
                            player_cxts.insert(cxt.id(), Arc::clone(&cxt));
                            if let Some(diffs) = hooks.join(cxt.as_ref()) {
                                for diff in diffs {
                                    match diff {
                                        Diff::All { delta } => {
                                            let delta = DiffNotification::new(
                                                self.type_,
                                                self.id.as_str(),
                                                delta.serialize(),
                                            )
                                            .serialize();

                                            for p_id in player_cxts.keys() {
                                                session_manager.send(*p_id, delta.clone());
                                            }
                                        }
                                        Diff::Target { ids, delta } => {
                                            let delta = DiffNotification::new(
                                                self.type_,
                                                self.id.as_str(),
                                                delta.serialize(),
                                            )
                                            .serialize();

                                            for &p_id in ids.iter() {
                                                session_manager.send(p_id, delta.clone());
                                            }
                                        }
                                    }
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

                for diff in hooks.diff(&player_cxts, actions_buffer.as_slice()) {
                    match diff {
                        Diff::All { delta } => {
                            let delta = DiffNotification::new(
                                self.type_,
                                self.id.as_str(),
                                delta.serialize(),
                            )
                            .serialize();

                            for p_id in player_cxts.keys() {
                                session_manager.send(*p_id, delta.clone());
                            }
                        }
                        Diff::Target { ids, delta } => {
                            let delta = DiffNotification::new(
                                self.type_,
                                self.id.as_str(),
                                delta.serialize(),
                            )
                            .serialize();

                            for &p_id in ids.iter() {
                                session_manager.send(p_id, delta.clone());
                            }
                        }
                    }
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
                println!("SERVER received action: {:?}", action);
            }
            Event::Join(cxt) => {
                println!("SERVER received join: {:?}", cxt);
            }

            Event::Leave(id) => {
                println!("SERVER received leave: {:?}", id);
            }
        }

        if let Err(_) = self.actions.send((p_id, event)) {
            println!("Runtime Not Connected. Skipping Action.");
        }
    }
}
