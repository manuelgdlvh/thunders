use std::{
    collections::{BinaryHeap, HashMap},
    hash::Hash,
    sync::{Mutex, RwLock, atomic::AtomicBool},
    time::{Duration, Instant},
};

use tokio::sync::oneshot::{self, Receiver, Sender};

pub enum Reply<R, E> {
    Ok(R),
    Err(E),
    Timeout,
}

#[derive(PartialEq, Eq)]
struct RegisteredTimeout<Id>
where
    Id: Eq + Hash,
{
    id: Id,
    expires_at: Instant,
}

impl<Id> Ord for RegisteredTimeout<Id>
where
    Id: Eq + Hash,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.expires_at.cmp(&self.expires_at)
    }
}

impl<Id> PartialOrd for RegisteredTimeout<Id>
where
    Id: PartialEq + Eq + Hash,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct ReplyManager<Id, R, E>
where
    Id: Eq + Hash,
{
    replies_registry: Mutex<HashMap<Id, Sender<Reply<R, E>>>>,
    registered_timeouts: RwLock<BinaryHeap<RegisteredTimeout<Id>>>,
    // TODO: improve this using custom wakers
    tick_interval: tokio::time::Duration,
}

impl<Id, R, E> ReplyManager<Id, R, E>
where
    Id: Eq + Hash + Clone,
{
    pub fn new(tick_interval: tokio::time::Duration) -> Self {
        Self {
            replies_registry: Mutex::new(HashMap::new()),
            registered_timeouts: RwLock::new(BinaryHeap::new()),
            tick_interval,
        }
    }

    pub fn register(&self, id: impl Into<Id>, expires_in: Duration) -> Receiver<Reply<R, E>> {
        let (tx, rx) = oneshot::channel::<Reply<R, E>>();

        let id = id.into();
        self.replies_registry
            .lock()
            .expect("Should lock always be acquirable")
            .insert(id.clone(), tx);

        self.registered_timeouts
            .write()
            .expect("Should write lock always be acquirable")
            .push(RegisteredTimeout {
                id,
                expires_at: Instant::now()
                    .checked_add(expires_in)
                    .expect("Should expires never overflow internal structure"),
            });
        rx
    }

    pub fn ok(&self, id: &Id, result: R) {
        if let Some(pending_reply) = self.replies_registry.lock().expect("").remove(id) {
            let _ = pending_reply.send(Reply::Ok(result));
        }
    }

    pub fn error(&self, id: &Id, error: E) {
        if let Some(pending_reply) = self.replies_registry.lock().expect("").remove(id) {
            let _ = pending_reply.send(Reply::Err(error));
        }
    }

    pub fn vacuum(&self) {
        let now = Instant::now();
        loop {
            if let Some(registered_timeout) = self
                .registered_timeouts
                .read()
                .expect("Should read lock always be acquirable")
                .peek()
            {
                if now < registered_timeout.expires_at {
                    break;
                }
            } else {
                break;
            }

            let registered_timeout = self
                .registered_timeouts
                .write()
                .expect("Should write lock always be acquirable")
                .pop()
                .expect("Should always exists timeout entry if checked before");

            if let Some(pending_reply) = self
                .replies_registry
                .lock()
                .expect("")
                .remove(&registered_timeout.id)
            {
                let _ = pending_reply.send(Reply::Timeout);
            }
        }
    }
}
