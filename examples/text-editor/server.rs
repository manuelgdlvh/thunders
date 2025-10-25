use std::{collections::HashMap, sync::Arc};

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
        .register::<SyncRuntime<_>, TextEditor>(
            LOBBY_TYPE,
            Settings {
                tick_no_action_millis: 2000,
                tick_millis: 16,
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

impl thunders::server::hooks::GameHooks for TextEditor {
    type Delta = TextEditorChange;
    type Action = TextEditorAction;
    type Options = ();

    fn build(options: Self::Options) -> Self {
        Self {
            content: Default::default(),
        }
    }

    fn on_join(&mut self, player_cxt: &PlayerContext) -> Option<Vec<Diff<Self::Delta>>> {
        Some(vec![Diff::TargetUnique {
            id: player_cxt.id(),
            delta: TextEditorChange::Full(self.content.to_string()),
        }])
    }

    fn on_leave(&mut self, player_cxt: &PlayerContext) -> Option<Diff<Self::Delta>> {
        None
    }

    fn on_tick(
        &mut self,
        players_cxts: &HashMap<u64, Arc<PlayerContext>>,
        mut actions: Vec<(u64, Self::Action)>,
    ) -> Option<Vec<Diff<Self::Delta>>> {
        if actions.is_empty() {
            None
        } else {
            let (p_id, action) = actions.pop().expect("Should have at least one action");
            match action {
                TextEditorAction::TextReplace(text) => {
                    self.content = text;
                }
            }

            Some(vec![Diff::TargetList {
                ids: players_cxts
                    .keys()
                    .filter(|id| !p_id.eq(*id))
                    .map(|id| *id)
                    .collect::<Vec<_>>(),
                delta: TextEditorChange::Full(self.content.clone()),
            }])
        }
    }

    fn is_finished(&self) -> (bool, Option<Diff<Self::Delta>>) {
        (false, None)
    }
}
