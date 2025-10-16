use crate::api::message::OutputMessage;
use std::error::Error;
use std::fmt::Display;

impl<'a> From<ThundersServerError> for OutputMessage<'a> {
    fn from(val: ThundersServerError) -> Self {
        let description = match val {
            _ => "Generic error, please provide more details",
        };
        OutputMessage::GenericError { description }
    }
}

#[derive(Debug)]
pub enum ThundersServerError {
    StartFailure,
    MessageNotConnected,
    RoomNotFound,
    RoomAlreadyCreated,
    RoomTypeNotFound,
    ConnectionFailure,
    InvalidInput,
    DeserializationFailure,
}

impl Display for ThundersServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl Error for ThundersServerError {}
