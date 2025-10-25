use std::{collections::HashMap, env, sync::mpsc, thread, time::Duration};

use ::rand::{RngCore, thread_rng};
use macroquad::prelude::*;
use thunders::{
    api::schema::json::Json,
    client::{ThundersClient, ThundersClientBuilder, protocol::ws::WebSocketClientProtocol},
    server::{
        ThundersServer,
        context::PlayerContext,
        hooks::Diff,
        protocol::ws::WebSocketProtocol,
        runtime::sync::{Settings, SyncRuntime},
    },
};
use tokio::runtime::Builder;

const SCR_W: f32 = 20.0;
const SCR_H: f32 = 20.0;
const PLATFORM_WIDTH: f32 = 0.2;
const PLATFORM_HEIGHT: f32 = 3.;
const DELTA: f32 = 0.016;

const SERVER_MODE: &str = "server";
const LOBBY_TYPE: &str = "pong";
const LOBBY_ID: &str = "pong_1";

#[macroquad::main("Pong")]
async fn main() {
    let args = env::args().collect::<Vec<String>>();
    let mode = args
        .get(1)
        .expect("Add server or client mode as argument. Example: -- client");

    let client = if mode == SERVER_MODE {
        start_server();
        start_client(true)
    } else {
        start_client(false)
    };

    set_camera(&Camera2D {
        zoom: vec2(1. / SCR_W * 2., 1. / SCR_H * 2.),
        target: vec2(SCR_W / 2., SCR_H / 2.),
        ..Default::default()
    });

    loop {
        clear_background(DARKPURPLE);

        let mut movement = None;

        if let Some(game) = client
            .active_games
            .get_as::<PongGame>(LOBBY_TYPE, LOBBY_ID)
            .expect("Room type should always exists")
        {
            let game = game.as_ref();
            if is_key_down(KeyCode::Up) && game.own_platform.1 > PLATFORM_HEIGHT / 2. {
                movement = Some(PongAction::MovePlatformUp);
            }
            if is_key_down(KeyCode::Down) && game.own_platform.1 < SCR_H - PLATFORM_HEIGHT / 2. {
                movement = Some(PongAction::MovePlatformDown);
            }

            draw_circle(SCR_W / 2., SCR_H / 2., 0.3, BLACK);
            draw_line(SCR_W / 2., 0., SCR_W / 2., 7., 0.12, BLACK);
            draw_line(SCR_W / 2., SCR_H, SCR_W / 2., SCR_H - 7., 0.12, BLACK);
            draw_circle_lines(SCR_W / 2., SCR_H / 2., 3., 0.15, BLACK);

            draw_circle(game.ball.position.0, game.ball.position.1, 0.2, BLUE);

            draw_rectangle(
                game.own_platform.0,
                game.own_platform.1 - PLATFORM_HEIGHT / 2.,
                PLATFORM_WIDTH,
                PLATFORM_HEIGHT,
                PURPLE,
            );

            if let Some(away_platform) = game.away_platform.as_ref() {
                draw_rectangle(
                    away_platform.0,
                    away_platform.1 - PLATFORM_HEIGHT / 2.,
                    PLATFORM_WIDTH,
                    PLATFORM_HEIGHT,
                    PURPLE,
                );
            }
        } else {
            break;
        }

        if let Some(movement) = movement {
            let _ = client.action::<PongGame>(LOBBY_TYPE, LOBBY_ID, movement);
        }

        next_frame().await
    }
}

fn start_server() {
    thread::spawn(|| {
        let rt = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("server-runtime")
            .build()
            .expect("failed to build Tokio runtime");

        rt.block_on(async {
            let _ = ThundersServer::new(WebSocketProtocol::new("127.0.0.1", 8080), Json::default())
                .register::<SyncRuntime<_>, PongServer>(
                    LOBBY_TYPE,
                    Settings {
                        tick_no_action_millis: (DELTA * 1000.0) as u64,
                        tick_millis: (DELTA * 1000.0) as u64,
                    },
                )
                .run()
                .await;
        });
    });
}

fn start_client(create_game: bool) -> ThundersClient<Json> {
    let (tx, rx) = mpsc::sync_channel::<ThundersClient<Json>>(0);
    thread::spawn(move || {
        let rt = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("client-runtime")
            .build()
            .expect("failed to build Tokio runtime");

        rt.block_on(async {
            let client = ThundersClientBuilder::new(
                WebSocketClientProtocol::new("127.0.0.1", 8080),
                Json::default(),
            )
            .register(LOBBY_TYPE)
            .build()
            .await
            .unwrap();

            let player_id: u64 = thread_rng().next_u64();
            client
                .connect(player_id, Duration::from_secs(5))
                .await
                .unwrap();
            if create_game {
                client
                    .create::<PongGame>(
                        LOBBY_TYPE,
                        LOBBY_ID,
                        Default::default(),
                        Duration::from_secs(5),
                    )
                    .await
                    .unwrap();
            } else {
                client
                    .join::<PongGame>(LOBBY_TYPE, LOBBY_ID, Duration::from_secs(5))
                    .await
                    .unwrap();
            }
            tx.send(client).unwrap();
            std::future::pending::<()>().await;
        });
    });

    rx.recv().unwrap()
}

// State

struct Ball {
    position: BallPosition,
    vector: BallVector,
}
struct BallPosition(f32, f32);
struct BallVector(f32, f32);

struct PlatformPosition(f32, f32);

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PongAction {
    MovePlatformUp,
    MovePlatformDown,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PongDiff {
    BallVectorChanged(f32, f32),
    BallPositionChanged(f32, f32),
    Full {
        ball_position: (f32, f32),
        ball_vector: (f32, f32),
        own_position: (f32, f32),
        away_position: (f32, f32),
    },
    AwayJoined(f32, f32),
    AwayPositionChanged(f32),
}

// Server

pub struct PongServer {
    ball: Ball,
    platforms: HashMap<u64, PlatformPosition>,
    platform_host_id: Option<u64>,
    started: bool,
}

impl thunders::server::hooks::GameHooks for PongServer {
    type Options = ();
    type Action = PongAction;
    type Delta = PongDiff;

    fn build(options: Self::Options) -> Self {
        Self {
            ball: Ball {
                position: BallPosition(12., 7.),
                vector: BallVector(3.5, -3.5),
            },
            platforms: HashMap::new(),
            platform_host_id: None,
            started: false,
        }
    }

    fn on_join(&mut self, player_cxt: &PlayerContext) -> Option<Vec<Diff<Self::Delta>>> {
        if self.platform_host_id.is_none() {
            self.platform_host_id = Some(player_cxt.id());
            self.platforms
                .insert(player_cxt.id(), PlatformPosition(0., 12.));
            None
        } else {
            let mut result = vec![];

            let away_pos = self
                .platforms
                .get(self.platform_host_id.as_ref().unwrap())
                .unwrap();
            result.push(Diff::TargetUnique {
                id: player_cxt.id(),
                delta: PongDiff::Full {
                    ball_position: (self.ball.position.0, self.ball.position.1),
                    ball_vector: (self.ball.vector.0, self.ball.vector.1),
                    own_position: (SCR_W - PLATFORM_WIDTH, 12.),
                    away_position: (away_pos.0, away_pos.1),
                },
            });

            result.push(Diff::TargetUnique {
                id: self.platform_host_id.unwrap(),
                delta: PongDiff::AwayJoined(SCR_W - PLATFORM_WIDTH, 12.),
            });

            self.platforms.insert(
                player_cxt.id(),
                PlatformPosition(SCR_W - PLATFORM_WIDTH, 12.),
            );

            self.started = true;
            Some(result)
        }
    }

    fn on_leave(&mut self, player_cxt: &PlayerContext) -> Option<Diff<Self::Delta>> {
        None
    }

    fn is_finished(&self) -> (bool, Option<Diff<Self::Delta>>) {
        (false, None)
    }

    fn on_tick(
        &mut self,
        player_cxts: &HashMap<u64, std::sync::Arc<PlayerContext>>,
        actions: Vec<(u64, Self::Action)>,
    ) -> Option<Vec<Diff<Self::Delta>>> {
        let mut diffs = vec![];
        for (p_id, action) in actions {
            match action {
                PongAction::MovePlatformUp => {
                    let ids: Vec<u64> = self
                        .platforms
                        .keys()
                        .filter(|id| !p_id.eq(*id))
                        .map(|id| *id)
                        .collect();
                    if let Some(platform) = self.platforms.get_mut(&p_id) {
                        platform.1 -= 10.0 * DELTA;
                        diffs.push(Diff::TargetList {
                            ids,
                            delta: PongDiff::AwayPositionChanged(platform.1),
                        });
                    }
                }

                PongAction::MovePlatformDown => {
                    let ids: Vec<u64> = self
                        .platforms
                        .keys()
                        .filter(|id| !p_id.eq(*id))
                        .map(|id| *id)
                        .collect();
                    if let Some(platform) = self.platforms.get_mut(&p_id) {
                        platform.1 += 10.0 * DELTA;
                        diffs.push(Diff::TargetList {
                            ids,
                            delta: PongDiff::AwayPositionChanged(platform.1),
                        });
                    }
                }
            }
        }

        if !self.started {
            return Some(diffs);
        }

        self.ball.position.0 += self.ball.vector.0 * DELTA;
        self.ball.position.1 += self.ball.vector.1 * DELTA;

        diffs.push(Diff::All {
            delta: PongDiff::BallPositionChanged(self.ball.position.0, self.ball.position.1),
        });

        let mut is_ball_vector_changed = false;
        if self.ball.position.1 <= 0. || self.ball.position.1 >= SCR_H {
            self.ball.vector.1 *= -1.;
            is_ball_vector_changed = true;
        }

        if (self.ball.position.0 <= PLATFORM_WIDTH && self.ball.vector.0 < 0.)
            || (self.ball.position.0 >= (SCR_W - PLATFORM_WIDTH) && self.ball.vector.0 > 0.)
        {
            is_ball_vector_changed = true;
            let mut ball_hit = false;
            for platform in self.platforms.values() {
                if 0.0.eq(&platform.0) && self.ball.vector.0 > 0. {
                    continue;
                }

                if (SCR_W - PLATFORM_WIDTH).eq(&platform.0) && self.ball.vector.0 < 0. {
                    continue;
                }

                if self.ball.position.1 >= platform.1 - PLATFORM_HEIGHT / 2. - 0.1
                    && self.ball.position.1 <= platform.1 + PLATFORM_HEIGHT / 2. + 0.1
                {
                    self.ball.vector.0 *= 1.15;
                    self.ball.vector.1 *= 1.15;
                    ball_hit = true;
                }
            }

            if ball_hit {
                self.ball.vector.0 *= -1.;
            } else {
                self.ball.vector.0 = 3.5;
                self.ball.vector.1 = 3.5;
                self.ball.position.0 = 12.;
                self.ball.position.1 = 7.;

                diffs.push(Diff::All {
                    delta: PongDiff::BallPositionChanged(12., 7.),
                });
            }
        }

        if is_ball_vector_changed {
            diffs.push(Diff::All {
                delta: PongDiff::BallVectorChanged(self.ball.vector.0, self.ball.vector.1),
            });
        }

        Some(diffs)
    }
}

// Client

pub struct PongGame {
    ball: Ball,
    own_platform: PlatformPosition,
    away_platform: Option<PlatformPosition>,
}

impl thunders::client::core::GameHooks for PongGame {
    type Change = PongDiff;
    type Action = PongAction;
    type Options = ();

    fn build(options: &Self::Options) -> Self {
        Self {
            ball: Ball {
                position: BallPosition(SCR_W / 2., SCR_H / 2.),
                vector: BallVector(3.5, -3.5),
            },
            own_platform: PlatformPosition(0., 12.),
            away_platform: None,
        }
    }

    fn on_change(&mut self, change: Self::Change) {
        match change {
            PongDiff::BallVectorChanged(x, y) => {
                self.ball.vector.0 = x;
                self.ball.vector.1 = y;
            }
            PongDiff::BallPositionChanged(x, y) => {
                self.ball.position.0 = x;
                self.ball.position.1 = y;
            }
            PongDiff::Full {
                ball_position,
                ball_vector,
                away_position,
                own_position,
            } => {
                self.ball.position.0 = ball_position.0;
                self.ball.position.1 = ball_position.1;
                self.ball.vector.0 = ball_vector.0;
                self.ball.vector.1 = ball_vector.1;
                self.own_platform = PlatformPosition(own_position.0, own_position.1);
                self.away_platform = Some(PlatformPosition(away_position.0, away_position.1));
            }
            PongDiff::AwayJoined(x, y) => {
                self.away_platform = Some(PlatformPosition(x, y));
            }
            PongDiff::AwayPositionChanged(y) => {
                if let Some(away_platform) = self.away_platform.as_mut() {
                    away_platform.1 = y;
                }
            }
        }
    }

    fn on_action(&mut self, action: Self::Action) {
        match action {
            PongAction::MovePlatformUp => {
                self.own_platform.1 -= DELTA * 10.0;
            }
            PongAction::MovePlatformDown => {
                self.own_platform.1 += DELTA * 10.0;
            }
        }
    }

    fn on_finish(self) {}
}
