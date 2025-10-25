use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc::Sender;
use futures::{SinkExt, Stream};
use iced::widget::text_editor;
use iced::{Alignment, Element, Length, Subscription, Task, stream};
use rand::RngCore;
use thunders::api::schema::json::Json;
use thunders::client::protocol::ws::WebSocketClientProtocol;
use thunders::client::{ThundersClient, ThundersClientBuilder};

const LOBBY_TYPE: &str = "text_editor";
const LOBBY_ID: &str = "text_editor_1";
const IP_ADDRESS: &str = "127.0.0.1";

fn main() {
    iced::application(Application::boot, Application::update, Application::view)
        .subscription(Application::subscription)
        .run()
        .unwrap()
}

#[derive(Default)]
pub struct Application {
    client: Option<Arc<ThundersClient<Json>>>,
    active: text_editor::Content,
}

#[derive(Debug, Clone)]
pub enum Event {
    ClientJoined,
    Connected(Arc<ThundersClient<Json>>),
    TextModified(text_editor::Action),
    CreationRequested,
    JoinRequested,
    Updated,
    Created,
    Joined,
}

impl Application {
    pub fn boot() -> (Self, Task<Event>) {
        (Self::default(), Task::none())
    }

    pub fn update(&mut self, message: Event) -> Task<Event> {
        match message {
            Event::Connected(client) => {
                self.client.replace(client);
                Task::none()
            }

            Event::CreationRequested => Task::future({
                let client = Arc::clone(self.client.as_ref().unwrap());
                async move {
                    let creation_fut = client.create::<TextEditor>(
                        LOBBY_TYPE,
                        LOBBY_ID,
                        Default::default(),
                        Duration::from_secs(5),
                    );

                    if creation_fut.await.is_err() {
                        panic!()
                    }

                    Event::Created
                }
            }),

            Event::JoinRequested => Task::future({
                let client = Arc::clone(self.client.as_ref().unwrap());
                async move {
                    let join_fut =
                        client.join::<TextEditor>(LOBBY_TYPE, LOBBY_ID, Duration::from_secs(5));

                    if join_fut.await.is_err() {
                        panic!()
                    }

                    Event::Joined
                }
            }),

            Event::TextModified(action) => {
                let should_sync = match action {
                    text_editor::Action::Edit(_) => true,
                    _ => false,
                };

                self.active.perform(action);

                if should_sync && let Some(client) = self.client.as_ref() {
                    let _ = client.action::<TextEditor>(
                        LOBBY_TYPE,
                        LOBBY_ID,
                        TextEditorAction::TextReplace(self.active.text()),
                    );
                }

                Task::none()
            }

            Event::Updated => {
                let client_ref = self.client.as_ref().expect("");
                let state_view_opt = client_ref
                    .active_games
                    .get_as::<TextEditor>(LOBBY_TYPE, LOBBY_ID)
                    .expect("Should room type exists always in this typed example");

                if let Some(view) = state_view_opt {
                    self.active = text_editor::Content::with_text(&view.as_ref().raw_text);
                }

                Task::none()
            }
            Event::ClientJoined | Event::Created | Event::Joined => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Event> {
        if self.client.is_some() {
            let create_btn = iced::widget::button("Create")
                .padding(10)
                .on_press(Event::CreationRequested);
            let join_btn = iced::widget::button("Join")
                .padding(10)
                .on_press(Event::JoinRequested);

            let text_editor = iced::widget::text_editor(&self.active)
                .on_action(Event::TextModified)
                .height(Length::Fill)
                .placeholder("Create, Join or Start typing");

            iced::widget::column!(
                iced::widget::row![create_btn, join_btn]
                    .width(Length::Fill)
                    .spacing(25),
                text_editor
            )
            .spacing(15)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            iced::widget::text!("Not Connected :(")
                .width(Length::Fill)
                .height(Length::Fill)
                .size(36)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
        }
    }

    fn listener() -> impl Stream<Item = Event> {
        let mut rng = rand::thread_rng();
        let id: u64 = rng.next_u64();
        stream::channel(100, move |mut output: Sender<Event>| async move {
            let client = ThundersClientBuilder::new(
                WebSocketClientProtocol::new(IP_ADDRESS, 8080),
                Json::default(),
            )
            .register(LOBBY_TYPE)
            .build()
            .await
            .unwrap();

            client.connect(id, Duration::from_secs(5)).await.unwrap();

            let client = Arc::new(client);
            let _ = output.send(Event::Connected(Arc::clone(&client))).await;

            while let Ok(_) = client.consume_event().await {
                let _ = output.send(Event::Updated).await;
            }
        })
    }

    pub fn subscription(&self) -> Subscription<Event> {
        Subscription::run(Self::listener)
    }
}

// Thunders

pub struct TextEditor {
    raw_text: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum TextEditorAction {
    TextReplace(String),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum TextEditorChange {
    Full(String),
}

impl thunders::client::core::GameHooks for TextEditor {
    type Change = TextEditorChange;
    type Action = TextEditorAction;
    type Options = ();

    fn build(_options: &Self::Options) -> Self {
        Self {
            raw_text: Default::default(),
        }
    }

    fn on_change(&mut self, change: Self::Change) {
        match change {
            Self::Change::Full(text) => {
                self.raw_text = text;
            }
        }
    }

    fn on_action(&mut self, action: Self::Action) {
        match action {
            Self::Action::TextReplace(text) => {
                self.raw_text = text;
            }
        }
    }

    fn on_finish(self) {}
}
