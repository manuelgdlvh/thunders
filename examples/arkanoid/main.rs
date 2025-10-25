use std::{collections::HashMap, env, sync::mpsc, thread, time::Duration};

use ::rand::{RngCore, thread_rng};
use macroquad::prelude::*;
use thunders::{
    api::schema::json::Json,
    client::{ThundersClient, ThundersClientBuilder, protocol::ws::WebSocketClientProtocol},
    server::{
        ThundersServer,
        context::PlayerContext,
        hooks::{Diff, GameHooks},
        protocol::ws::WebSocketProtocol,
        runtime::sync::{Settings, SyncRuntime},
    },
};
use tokio::runtime::Builder;

const BLOCKS_W: usize = 15;
const BLOCKS_H: usize = 15;
const SCR_W: f32 = 20.0;
const SCR_H: f32 = 20.0;
const PLATFORM_WIDTH: f32 = 3.;
const PLATFORM_HEIGHT: f32 = 0.2;
const DELTA: f32 = 0.016;

const SERVER_MODE: &str = "server";
const LOBBY_TYPE: &str = "arkanoid";
const LOBBY_ID: &str = "arkanoid_1";

#[macroquad::main("Arkanoid")]
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
        clear_background(SKYBLUE);

        let mut movement = None;

        if let Some(game) = client
            .active_games
            .get_as::<ArkanoidGame>(LOBBY_TYPE, LOBBY_ID)
            .expect("Room type should always exists")
        {
            let game = game.as_ref();
            if is_key_down(KeyCode::Right) && game.own_platform.0 < SCR_W - PLATFORM_WIDTH / 2. {
                movement = Some(ArkanoidAction::MovePlatformRight);
            }
            if is_key_down(KeyCode::Left) && game.own_platform.0 > PLATFORM_WIDTH / 2. {
                movement = Some(ArkanoidAction::MovePlatformLeft);
            }

            for j in 0..BLOCKS_H {
                for i in 0..BLOCKS_W {
                    if game.blocks[j][i] {
                        let block_w = SCR_W / BLOCKS_W as f32;
                        let block_h = 7.0 / BLOCKS_H as f32;
                        let block_x = i as f32 * block_w + 0.05;
                        let block_y = j as f32 * block_h + 0.05;

                        draw_rectangle(block_x, block_y, block_w - 0.1, block_h - 0.1, DARKBLUE);
                    }
                }
            }

            draw_circle(game.ball.position.0, game.ball.position.1, 0.2, RED);
            draw_rectangle(
                game.own_platform.0 - PLATFORM_WIDTH / 2.,
                SCR_H - PLATFORM_HEIGHT,
                PLATFORM_WIDTH,
                PLATFORM_HEIGHT,
                DARKPURPLE,
            );

            if let Some(away_platform) = game.away_platform.as_ref() {
                draw_rectangle(
                    away_platform.0 - PLATFORM_WIDTH / 2.,
                    SCR_H - PLATFORM_HEIGHT,
                    PLATFORM_WIDTH,
                    PLATFORM_HEIGHT,
                    PURPLE,
                );
            }
        } else {
            break;
        }

        if let Some(movement) = movement {
            let _ = client.action::<ArkanoidGame>(LOBBY_TYPE, LOBBY_ID, movement);
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
                .register::<SyncRuntime<_>, ArkanoidServer>(
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
                    .create::<ArkanoidGame>(
                        LOBBY_TYPE,
                        LOBBY_ID,
                        Default::default(),
                        Duration::from_secs(5),
                    )
                    .await
                    .unwrap();
            } else {
                client
                    .join::<ArkanoidGame>(LOBBY_TYPE, LOBBY_ID, Duration::from_secs(5))
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

struct PlatformPosition(f32);

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ArkanoidAction {
    MovePlatformLeft,
    MovePlatformRight,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ArkanoidDiff {
    BlockDestroyed(usize, usize),
    BallVectorChanged(f32, f32),
    BallPositionChanged(f32, f32),
    Full {
        ball_position: (f32, f32),
        ball_vector: (f32, f32),
        blocks: [[bool; BLOCKS_W]; BLOCKS_H],
        away_position: f32,
    },
    AwayJoined(f32),
    AwayPositionChanged(f32),
}

// Server

pub struct ArkanoidServer {
    ball: Ball,
    platforms: HashMap<u64, PlatformPosition>,
    blocks: [[bool; BLOCKS_W]; BLOCKS_H],
    platform_host_id: Option<u64>,
}

impl thunders::server::hooks::GameHooks for ArkanoidServer {
    type Options = ();
    type Action = ArkanoidAction;
    type Delta = ArkanoidDiff;

    fn build(options: Self::Options) -> Self {
        Self {
            ball: Ball {
                position: BallPosition(12., 7.),
                vector: BallVector(3.5, -3.5),
            },
            platforms: HashMap::new(),
            blocks: [[true; BLOCKS_W]; BLOCKS_H],
            platform_host_id: None,
        }
    }

    fn on_join(&mut self, player_cxt: &PlayerContext) -> Option<Vec<Diff<Self::Delta>>> {
        if self.platform_host_id.is_none() {
            self.platform_host_id = Some(player_cxt.id());
            self.platforms
                .insert(player_cxt.id(), PlatformPosition(12.));
            None
        } else {
            let mut result = vec![];

            result.push(Diff::TargetUnique {
                id: player_cxt.id(),
                delta: ArkanoidDiff::Full {
                    ball_position: (self.ball.position.0, self.ball.position.1),
                    ball_vector: (self.ball.vector.0, self.ball.vector.1),
                    blocks: self.blocks,
                    away_position: self
                        .platforms
                        .get(self.platform_host_id.as_ref().unwrap())
                        .unwrap()
                        .0,
                },
            });

            result.push(Diff::TargetUnique {
                id: self.platform_host_id.unwrap(),
                delta: ArkanoidDiff::AwayJoined(12.),
            });

            self.platforms
                .insert(player_cxt.id(), PlatformPosition(12.));
            Some(result)
        }
    }

    fn on_leave(&mut self, player_cxt: &PlayerContext) -> Option<Diff<Self::Delta>> {
        None
    }

    fn is_finished(&self) -> (bool, Option<Diff<Self::Delta>>) {
        if self.ball.position.1 >= SCR_H {
            (true, None)
        } else {
            (false, None)
        }
    }

    fn on_tick(
        &mut self,
        player_cxts: &HashMap<u64, std::sync::Arc<PlayerContext>>,
        actions: Vec<(u64, Self::Action)>,
    ) -> Option<Vec<Diff<Self::Delta>>> {
        let mut diffs = vec![];
        for (p_id, action) in actions {
            match action {
                ArkanoidAction::MovePlatformLeft => {
                    let ids: Vec<u64> = self
                        .platforms
                        .keys()
                        .filter(|id| !p_id.eq(*id))
                        .map(|id| *id)
                        .collect();
                    if let Some(platform) = self.platforms.get_mut(&p_id) {
                        platform.0 -= 10.0 * DELTA;
                        diffs.push(Diff::TargetList {
                            ids,
                            delta: ArkanoidDiff::AwayPositionChanged(platform.0),
                        });
                    }
                }

                ArkanoidAction::MovePlatformRight => {
                    let ids: Vec<u64> = self
                        .platforms
                        .keys()
                        .filter(|id| !p_id.eq(*id))
                        .map(|id| *id)
                        .collect();
                    if let Some(platform) = self.platforms.get_mut(&p_id) {
                        platform.0 += 10.0 * DELTA;
                        diffs.push(Diff::TargetList {
                            ids,
                            delta: ArkanoidDiff::AwayPositionChanged(platform.0),
                        });
                    }
                }
            }
        }

        self.ball.position.0 += self.ball.vector.0 * DELTA;
        self.ball.position.1 += self.ball.vector.1 * DELTA;

        diffs.push(Diff::All {
            delta: ArkanoidDiff::BallPositionChanged(self.ball.position.0, self.ball.position.1),
        });

        let mut is_ball_vector_changed = false;
        if self.ball.position.0 <= 0. || self.ball.position.0 > SCR_W {
            self.ball.vector.0 *= -1.;
            is_ball_vector_changed = true;
        }

        for platform in self.platforms.values() {
            if self.ball.position.1 <= 0.
                || (self.ball.position.1 > SCR_H - PLATFORM_HEIGHT - 0.15 / 2.
                    && self.ball.position.0 >= platform.0 - PLATFORM_WIDTH / 2.
                    && self.ball.position.0 <= platform.0 + PLATFORM_WIDTH / 2.)
            {
                self.ball.vector.1 = -1.;
                is_ball_vector_changed = true;
            }
        }

        if is_ball_vector_changed {
            diffs.push(Diff::All {
                delta: ArkanoidDiff::BallVectorChanged(self.ball.vector.0, self.ball.vector.1),
            });
        }

        for j in 0..BLOCKS_H {
            for i in 0..BLOCKS_W {
                if self.blocks[j][i] {
                    let block_w = SCR_W / BLOCKS_W as f32;
                    let block_h = 7.0 / BLOCKS_H as f32;
                    let block_x = i as f32 * block_w + 0.05;
                    let block_y = j as f32 * block_h + 0.05;

                    if self.ball.position.0 >= block_x
                        && self.ball.position.0 < block_x + block_w
                        && self.ball.position.1 >= block_y
                        && self.ball.position.1 < block_y + block_h
                    {
                        self.ball.vector.1 *= -1.;
                        self.blocks[j][i] = false;

                        diffs.push(Diff::All {
                            delta: ArkanoidDiff::BlockDestroyed(j, i),
                        });

                        diffs.push(Diff::All {
                            delta: ArkanoidDiff::BallVectorChanged(
                                self.ball.vector.0,
                                self.ball.vector.1,
                            ),
                        });

                        break;
                    }
                }
            }
        }

        Some(diffs)
    }
}

// Client

pub struct ArkanoidGame {
    ball: Ball,
    own_platform: PlatformPosition,
    away_platform: Option<PlatformPosition>,
    blocks: [[bool; BLOCKS_W]; BLOCKS_H],
}

impl thunders::client::core::GameHooks for ArkanoidGame {
    type Change = ArkanoidDiff;
    type Action = ArkanoidAction;
    type Options = ();

    fn build(options: &Self::Options) -> Self {
        Self {
            ball: Ball {
                position: BallPosition(12., 7.),
                vector: BallVector(3.5, -3.5),
            },
            own_platform: PlatformPosition(12.),
            away_platform: None,
            blocks: [[true; BLOCKS_W]; BLOCKS_H],
        }
    }

    fn on_change(&mut self, change: Self::Change) {
        match change {
            ArkanoidDiff::BlockDestroyed(i, j) => {
                self.blocks[i][j] = false;
            }
            ArkanoidDiff::BallVectorChanged(x, y) => {
                self.ball.vector.0 = x;
                self.ball.vector.1 = y;
            }
            ArkanoidDiff::BallPositionChanged(x, y) => {
                self.ball.position.0 = x;
                self.ball.position.1 = y;
            }
            ArkanoidDiff::Full {
                ball_position,
                ball_vector,
                blocks,
                away_position,
            } => {
                self.ball.position.0 = ball_position.0;
                self.ball.position.1 = ball_position.1;
                self.ball.vector.0 = ball_vector.0;
                self.ball.vector.1 = ball_vector.1;
                self.blocks = blocks;
                self.away_platform = Some(PlatformPosition(away_position));
            }
            ArkanoidDiff::AwayJoined(pos) => {
                self.away_platform = Some(PlatformPosition(pos));
            }
            ArkanoidDiff::AwayPositionChanged(pos) => {
                self.away_platform = Some(PlatformPosition(pos));
            }
        }
    }

    fn on_action(&mut self, action: Self::Action) {
        match action {
            ArkanoidAction::MovePlatformLeft => {
                self.own_platform.0 -= DELTA * 10.0;
            }
            ArkanoidAction::MovePlatformRight => {
                self.own_platform.0 += DELTA * 10.0;
            }
        }
    }

    fn on_finish(self) {}
}
