#[derive(Debug)]
pub enum ThundersClientError {
    ConnectionFailure,
    NotRunning,
    RoomNotFound,
    RoomTypeNotFound,
    UnknownMessage,
    NoResponse,
    IncompatibleAction,
    GameJoinFailure,
    GameCreationFailure,
}
