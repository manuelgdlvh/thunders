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
        hooks::{Diff, Event, GameHooks},
    },
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
        let (actions_tx, actions_rx) = mpsc::channel::<(u64, Event<H>)>();
        let r_handle = thread::spawn(move || {
            let mut actions_buffer = Vec::new();
            let mut now;
            let mut tick;

            // TODO: Add passive streaming actions. This is useful for example if you create a game with a maximum duration of 1 minute, you want prcess a Stop action that is not triggered by a user and is generated after 1 minute elapsed.
            // TODO: Add runtime hooks to handle connection, disconnection, join
            //
            let mut player_cxts: HashMap<u64, Arc<PlayerContext>> = HashMap::default();
            loop {
                if let Ok(event) = actions_rx.recv() {
                    match event.1 {
                        Event::Action(action) => {
                            actions_buffer.push((event.0, action));
                            now = Instant::now();
                            tick = Duration::from_millis(1000);
                        }

                        Event::Leave(id) => {
                            let player_context = player_cxts.remove(&id).unwrap();
                            if let Some(diff) = hooks.leave(player_context.as_ref()) {
                                match diff {
                                    Diff::All { delta } => {
                                        let delta = delta.serialize();
                                        for p_id in player_cxts.keys() {
                                            session_manager.send(*p_id, delta.clone());
                                        }
                                    }
                                    Diff::Target { ids, delta } => {
                                        let delta = delta.serialize();
                                        for &p_id in ids.iter() {
                                            session_manager.send(p_id, delta.clone());
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
                                            let delta = delta.serialize();
                                            for p_id in player_cxts.keys() {
                                                session_manager.send(*p_id, delta.clone());
                                            }
                                        }
                                        Diff::Target { ids, delta } => {
                                            let delta = delta.serialize();
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
                    break;
                }

                while let Ok(event) = actions_rx.recv_timeout(tick) {
                    match event.1 {
                        Event::Action(action) => {
                            actions_buffer.push((event.0, action));
                        }
                        Event::Leave(id) => {
                            let player_context = player_cxts.remove(&id).unwrap();
                            if let Some(diff) = hooks.leave(player_context.as_ref()) {
                                match diff {
                                    Diff::All { delta } => {
                                        let delta = delta.serialize();
                                        for p_id in player_cxts.keys() {
                                            session_manager.send(*p_id, delta.clone());
                                        }
                                    }
                                    Diff::Target { ids, delta } => {
                                        let delta = delta.serialize();
                                        for &p_id in ids.iter() {
                                            session_manager.send(p_id, delta.clone());
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
                                            let delta = delta.serialize();
                                            for p_id in player_cxts.keys() {
                                                session_manager.send(*p_id, delta.clone());
                                            }
                                        }
                                        Diff::Target { ids, delta } => {
                                            let delta = delta.serialize();
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
                            let delta = delta.serialize();
                            for p_id in player_cxts.keys() {
                                session_manager.send(*p_id, delta.clone());
                            }
                        }
                        Diff::Target { ids, delta } => {
                            let delta = delta.serialize();
                            for &p_id in ids.iter() {
                                session_manager.send(p_id, delta.clone());
                            }
                        }
                    }
                }

                hooks.update(mem::take(&mut actions_buffer));

                if hooks.is_finished() {
                    break;
                }
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
        self.actions.send((p_id, event)).expect("");
    }
}
