use crate::api::message::OutputMessage;
use std::error::Error;
use std::{borrow::Cow, fmt::Display};

impl<'a> Into<OutputMessage<'a>> for ThundersServerError {
    fn into(self) -> OutputMessage<'a> {
        let description = match self {
            _ => "Generic error, please provide more details",
        };
        OutputMessage::GenericError {
            description: Cow::Borrowed(description),
        }
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
