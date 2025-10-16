use std::error::Error;
use std::fmt::Display;

use crate::api::message::OutputMessage;

impl<'a> From<ThundersError> for OutputMessage<'a> {
    fn from(val: ThundersError) -> Self {
        let description = match val {
            _ => "Generic error, please provide more details",
        };
        OutputMessage::GenericError { description }
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
