// This framework provides the skeleton for efficient communication, client–time synchronization, and consistent state management in a high-performance multiplayer game architecture.

// Key design goals:

// Highly configurable: Adaptable to different game genres and modes.

// State synchronization: Keep the authoritative state consistent with client commands.

// Rollback support: Detect and correct out-of-sync actions through rollback and resimulation.

// Networking efficiency: Minimize bandwidth usage via techniques such as delta updates, snapshot compression, and interest management.

// Transport agnostic: The core remains independent of the underlying communication protocol (WebSockets, UDP, reliable UDP, QUIC, etc.) and of horizontal scaling across nodes.

// Layered Architecture

// Transport Layer
// Handles raw communication between server and clients. Protocol-agnostic design allows plug-in of different transports (UDP, TCP, WebRTC, QUIC).

// Event Layer
// Filters, transforms, and routes events (e.g., input commands, system signals, heartbeat messages). Provides hooks for middleware (e.g., anti-cheat, replay logging, telemetry).

// State Layer
// Maintains the ephemeral interaction state between clients and their actions.

// Example: A football match, a lobby room, or a raid instance.

// Tracks participants, authority, and lifecycle.

// Exposes utilities such as timers (e.g., match countdown, auto-close lobbies).

// Encourages composability: stages can be combined or extended with custom configurations.

// Essential Features Found in Modern Frameworks

// Deterministic Simulation
// For rollback netcode or lockstep protocols, ensuring consistent results across clients when inputs are replayed.

// Entity/Component System
// Authoritative server maintains the game world; clients receive entity snapshots or deltas.

// Client Prediction + Server Reconciliation
// Clients simulate immediate results of input to reduce latency, while the server periodically corrects them.

// Interest Management
// Clients receive only relevant state updates (e.g., nearby entities, visible objects) to reduce bandwidth.

// Tick-based Simulation
// Fixed update loops with sequence numbers for time synchronization.

// Snapshot Interpolation
// Clients render smooth gameplay by interpolating between past server snapshots.

// Rollback & Resimulation
// Especially used in fighting games and RTS: if a late input arrives, the system rolls back to the affected tick, replays inputs, and reapplies updates.

// Horizontal Scalability
// Support for sharding and room-based partitioning across nodes, with optional state handover for migrating sessions.

// Persistence Layer Integration
// Allows durable storage of player progress, inventories, and meta-game data.

// Security Hooks
// Anti-cheat verification, authority delegation, and rate-limiting at the event layer.

// Observability & Replay
// Logging, replay tools, and metrics (tick rate, lag spikes, rollback frequency) for debugging and balancing.

use std::{collections::HashMap, sync::Arc};

use crate::{
    hooks::GameHooks,
    protocol::{InputMessage, NetworkProtocol, SessionManager},
    runtime::{GameRuntime, GameRuntimeAnyHandle, GameRuntimeHandle},
    schema::{DeSerialize, Schema},
};

pub mod hooks;
pub mod protocol;
pub mod runtime;
pub mod schema;

pub struct MultiPlayer<N, S>
where
    N: NetworkProtocol,
    S: Schema,
{
    protocol: N,
    schema: S,
    handlers: HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>>,
    session_manager: Arc<SessionManager>,
}

impl<N, S> MultiPlayer<N, S>
where
    N: NetworkProtocol,
    S: Schema + 'static,
{
    pub fn new(protocol: N, schema: S) -> Self {
        Self {
            protocol,
            schema,
            handlers: Default::default(),
            session_manager: Arc::new(SessionManager::default()),
        }
    }

    pub fn register<R: GameRuntime<H, S> + 'static, H: GameHooks>(
        mut self,
        type_: &'static str,
    ) -> Self
    where
        H::Delta: DeSerialize<S>,
        H::Options: DeSerialize<S>,
        H::Action: DeSerialize<S>,
    {
        self.handlers.insert(
            type_,
            Box::new(GameRuntimeHandle::<R, H, S>::new(Arc::clone(
                &self.session_manager,
            ))),
        );
        self
    }

    pub async fn run(self)
    where
        InputMessage: DeSerialize<S>,
    {
        let handlers: &'static HashMap<&'static str, Box<dyn GameRuntimeAnyHandle>> =
            Box::leak(Box::new(self.handlers));

        self.protocol.run::<S>(self.session_manager, handlers).await;
    }
}
