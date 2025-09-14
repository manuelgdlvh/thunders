use thunders::{
    MultiPlayer,
    hooks::GameHooks,
    protocol::WebSocketProtocol,
    runtime::SyncGameRuntime,
    schema::{DeSerialize, Json},
};

#[tokio::main]
pub async fn main() {
    MultiPlayer::new(
        WebSocketProtocol {
            addr: "127.0.0.1:8080",
        },
        Json {},
    )
    .register::<SyncGameRuntime, Chat>("chat")
    .run()
    .await;
}

pub struct Chat {
    _messages: Vec<String>,
}

impl GameHooks for Chat {
    type Delta = ChatDiff;
    type Action = ChatAction;
    type Options = ();

    fn build(_options: Self::Options) -> Self {
        Self { _messages: vec![] }
    }

    fn diff(&self, _actions: &[Self::Action]) -> Self::Delta {
        let mut messages = Vec::new();

        for action in _actions {
            match action {
                ChatAction::IncomingMessage(message) => {
                    messages.push(message.clone());
                }
            }
        }

        ChatDiff { messages }
    }

    fn update(&mut self, _actions: Vec<Self::Action>) {
        for action in _actions.into_iter() {
            match action {
                ChatAction::IncomingMessage(message) => {
                    self._messages.push(message);
                }
            }
        }
    }
    fn merge(&self, _actions: Vec<Self::Delta>) -> Vec<Self::Delta> {
        vec![]
    }

    fn is_finished(&self) -> bool {
        false
    }
}

// Action
pub enum ChatAction {
    IncomingMessage(String),
}

impl DeSerialize<Json> for ChatAction {
    fn deserialize(_value: Vec<u8>) -> Result<Self, std::io::Error> {
        Ok(Self::IncomingMessage("test".to_string()))
    }

    fn serialize(self) -> Vec<u8> {
        unreachable!()
    }
}

// Delta

pub struct ChatDiff {
    messages: Vec<String>,
}

impl DeSerialize<Json> for ChatDiff {
    fn deserialize(_value: Vec<u8>) -> Result<Self, std::io::Error> {
        unreachable!()
    }

    fn serialize(self) -> Vec<u8> {
        self.messages.join(",").into_bytes()
    }
}
