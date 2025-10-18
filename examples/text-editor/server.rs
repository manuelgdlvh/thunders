use std::{collections::HashMap, sync::Arc};

use iced::widget::sensor::Key;
use thunders::{
    api::schema::json::Json,
    server::{
        ThundersServer, ThundersServerResult,
        context::PlayerContext,
        hooks::{Diff, GameHooks},
        protocol::ws::WebSocketProtocol,
        runtime::sync::{Settings, SyncRuntime},
    },
};

const LOBBY_TYPE: &str = "text_editor";
const LOBBY_ID: &str = "text_editor_1";
const IP_ADDRESS: &str = "127.0.0.1";

#[tokio::main]
pub async fn main() -> ThundersServerResult {
    ThundersServer::new(WebSocketProtocol::new(IP_ADDRESS, 8080), Json::default())
        .register::<SyncRuntime, TextEditor>(
            LOBBY_TYPE,
            Settings {
                max_action_await_millis: 2000,
                tick_interval_millis: 16,
            },
        )
        .run()
        .await
}

pub struct TextEditor {
    content: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum TextEditorAction {
    TextReplace(String),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum TextEditorChange {
    Full(String),
}

impl GameHooks for TextEditor {
    type Delta = TextEditorChange;
    type Action = TextEditorAction;
    type Options = ();

    fn build(options: Self::Options) -> Self {
        Self {
            content: Default::default(),
        }
    }

    fn diff(
        &self,
        player_cxts: &HashMap<u64, Arc<PlayerContext>>,
        actions: &[(u64, Self::Action)],
    ) -> Vec<Diff<Self::Delta>> {
        let text_ref = actions.last().expect("Should have at least one action");

        match &text_ref.1 {
            TextEditorAction::TextReplace(text) => {
                vec![Diff::Target {
                    ids: player_cxts
                        .keys()
                        .filter(|id| !text_ref.0.eq(*id))
                        .map(|id| *id)
                        .collect::<Vec<_>>(),
                    delta: TextEditorChange::Full(text.to_string()),
                }]
            }
        }
    }

    fn join(&self, player_cxt: &PlayerContext) -> Option<Vec<Diff<Self::Delta>>> {
        Some(vec![Diff::Target {
            ids: vec![player_cxt.id()],
            delta: TextEditorChange::Full(self.content.to_string()),
        }])
    }

    fn leave(&self, player_cxt: &PlayerContext) -> Option<Diff<Self::Delta>> {
        None
    }

    fn update(&mut self, mut actions: Vec<(u64, Self::Action)>) {
        println!("Processing update");
        match actions.pop().expect("Should have at least one action").1 {
            TextEditorAction::TextReplace(text) => {
                self.content = text;
            }
        }
    }

    fn finish(&self) -> (bool, Option<Diff<Self::Delta>>) {
        (false, None)
    }
}
