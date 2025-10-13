#[derive(Debug)]
pub enum ThundersClientError {
    ConnectionFailure,
    NotRunning,
    RoomNotFound,
    RoomTypeNotFound,
    UnknownMessage,
    NoResponse,
    GameJoinFailure,
    GameCreationFailure,
}
