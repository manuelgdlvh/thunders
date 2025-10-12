use std::borrow::Cow;
use std::error::Error;
use std::fmt::Display;

use crate::api::message::OutputMessage;

impl<'a> Into<OutputMessage<'a>> for ThundersError {
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
pub enum ThundersError {
    DeserializationFailure,
}

impl Display for ThundersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl Error for ThundersError {}
