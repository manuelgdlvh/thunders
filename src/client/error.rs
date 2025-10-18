#[derive(Debug)]
pub enum ThundersClientError {
    ConnectionFailure,
    RoomNotFound,
    RoomTypeNotFound,
    UnknownMessage,
    NoResponse,
    IncompatibleAction,
    GameJoinFailure,
    GameCreationFailure,
    EventListenerNotConfigured,
}
