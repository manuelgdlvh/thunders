use std::borrow::Cow;

use serde_json::Value;

use crate::api::{
    error::ThundersError,
    message::{InputMessage, OutputMessage},
    schema::{Deserialize, Schema, SchemaType, Serialize},
};

#[derive(Default)]
pub struct Json {}

impl Schema for Json {
    fn schema_type() -> SchemaType {
        SchemaType::Text
    }
}

impl<T> Serialize<Json> for T
where
    T: serde::Serialize,
{
    fn serialize(self) -> Vec<u8> {
        serde_json::to_vec(&self).expect("Should always be serializable")
    }
}

impl<T> Deserialize<Json> for T
where
    for<'de> T: serde::Deserialize<'de>,
{
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError> {
        serde_json::from_slice(&value).map_err(|_| ThundersError::DeserializationFailure)
    }
}

impl Serialize<Json> for InputMessage {
    fn serialize(self) -> Vec<u8> {
        match self {
            Self::Connect { correlation_id, id } => serde_json::json!({
                "method": "connect",
                "correlation_id": correlation_id,
                "id": id
            })
            .to_string()
            .into_bytes(),
            Self::Create { type_, id, .. } => serde_json::json!({
                "method": "create",
                "type": type_,
                "id": id
            })
            .to_string()
            .into_bytes(),
            Self::Join { type_, id } => serde_json::json!({
                "method": "join",
                "type": type_,
                "id": id
            })
            .to_string()
            .into_bytes(),

            Self::Action {
                type_,
                id,
                mut data,
            } => {
                let json = format!(
                    r#"
            {{
            "method": "action",
            "type" : "{}",
            "id": "{}",
            "data":               
            "#,
                    type_, id
                );

                let mut raw_message = json.into_bytes();
                raw_message.append(&mut data);
                raw_message.push(b'}');
                raw_message
            }
        }
    }
}

impl Deserialize<Json> for InputMessage {
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError> {
        const METHOD: &str = "method";
        const CORRELATION_ID: &str = "correlation_id";
        const CONNECT: &str = "connect";
        const ID: &str = "id";
        const CREATE: &str = "create";
        const TYPE: &str = "type";
        const OPTIONS: &str = "options";
        const DATA: &str = "data";
        const ACTION: &str = "action";
        const JOIN: &str = "join";

        let json: Value = serde_json::from_slice(value.as_slice())
            .map_err(|_| ThundersError::DeserializationFailure)?;

        let method = json
            .get(METHOD)
            .ok_or(ThundersError::DeserializationFailure)?;
        match method
            .as_str()
            .ok_or(ThundersError::DeserializationFailure)?
        {
            CONNECT => {
                let id = json.get(ID).ok_or(ThundersError::DeserializationFailure)?;
                let correlation_id = json
                    .get(CORRELATION_ID)
                    .ok_or(ThundersError::DeserializationFailure)?;
                Ok(InputMessage::Connect {
                    correlation_id: correlation_id
                        .as_str()
                        .ok_or(ThundersError::DeserializationFailure)?
                        .to_string(),
                    id: id.as_u64().ok_or(ThundersError::DeserializationFailure)?,
                })
            }
            CREATE => {
                let type_ = json
                    .get(TYPE)
                    .ok_or(ThundersError::DeserializationFailure)?;

                let id = json.get(ID).ok_or(ThundersError::DeserializationFailure)?;

                let options: Option<Vec<u8>> = json.get(OPTIONS).and_then(|node| {
                    serde_json::to_string(node)
                        .map(|json| json.into_bytes())
                        .ok()
                });

                Ok(InputMessage::Create {
                    type_: type_
                        .as_str()
                        .ok_or(ThundersError::DeserializationFailure)?
                        .to_string(),
                    id: id
                        .as_str()
                        .ok_or(ThundersError::DeserializationFailure)?
                        .to_string(),
                    options,
                })
            }

            JOIN => {
                let type_ = json
                    .get(TYPE)
                    .ok_or(ThundersError::DeserializationFailure)?;

                let id = json.get(ID).ok_or(ThundersError::DeserializationFailure)?;
                Ok(InputMessage::Join {
                    type_: type_
                        .as_str()
                        .ok_or(ThundersError::DeserializationFailure)?
                        .to_string(),
                    id: id
                        .as_str()
                        .ok_or(ThundersError::DeserializationFailure)?
                        .to_string(),
                })
            }
            ACTION => {
                let type_ = json
                    .get(TYPE)
                    .ok_or(ThundersError::DeserializationFailure)?;
                let id = json.get(ID).ok_or(ThundersError::DeserializationFailure)?;

                let data: Vec<u8> = serde_json::to_vec(
                    json.get(DATA)
                        .ok_or(ThundersError::DeserializationFailure)?,
                )
                .map_err(|_| ThundersError::DeserializationFailure)?;

                Ok(InputMessage::Action {
                    type_: type_
                        .as_str()
                        .ok_or(ThundersError::DeserializationFailure)?
                        .to_string(),
                    id: id
                        .as_str()
                        .ok_or(ThundersError::DeserializationFailure)?
                        .to_string(),
                    data,
                })
            }
            &_ => Err(ThundersError::DeserializationFailure),
        }
    }
}

// Output

const METHOD: &str = "method";
const CORRELATION_ID: &str = "correlation_id";

const CONNECT: &str = "connect";
const GENERIC_ERROR: &str = "generic_error";
const DIFF: &str = "diff";

const DATA: &str = "data";
const FINISHED: &str = "finished";
const TYPE: &str = "type";
const ID: &str = "id";
const DESCRIPTION: &str = "description";
const SUCCESS: &str = "success";

impl<'a> Serialize<Json> for OutputMessage<'a> {
    fn serialize(self) -> Vec<u8> {
        match self {
            OutputMessage::Connect {
                correlation_id,
                success,
            } => serde_json::json!({
                METHOD: CONNECT,
                CORRELATION_ID: correlation_id,
                SUCCESS: success
            }),

            OutputMessage::GenericError { description } => serde_json::json!({
                METHOD: GENERIC_ERROR,
                DESCRIPTION : description
            }),

            OutputMessage::Diff {
                type_,
                id,
                finished,
                data,
            } => {
                let mut json_node = serde_json::json!({
                    METHOD: DIFF,
                    TYPE: type_,
                    ID: id,
                    FINISHED: finished
                });

                if !data.is_empty() {
                    json_node
                        .as_object_mut()
                        .expect("Should always be a object")
                        .insert(
                            DATA.to_string(),
                            serde_json::from_slice::<Value>(data.as_slice())
                                .expect("Should always be serializable"),
                        );
                }

                json_node
            }
        }
        .to_string()
        .into_bytes()
    }
}

impl<'a> Deserialize<Json> for OutputMessage<'a> {
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError> {
        let json: Value = serde_json::from_slice(value.as_slice())
            .map_err(|_| ThundersError::DeserializationFailure)?;

        let method = json
            .get(METHOD)
            .ok_or(ThundersError::DeserializationFailure)?;
        match method
            .as_str()
            .ok_or(ThundersError::DeserializationFailure)?
        {
            CONNECT => {
                let success = json
                    .get(SUCCESS)
                    .ok_or(ThundersError::DeserializationFailure)?;
                let correlation_id = json
                    .get(CORRELATION_ID)
                    .ok_or(ThundersError::DeserializationFailure)?;
                Ok(OutputMessage::Connect {
                    correlation_id: Cow::Owned(
                        correlation_id
                            .as_str()
                            .ok_or(ThundersError::DeserializationFailure)?
                            .to_string(),
                    ),
                    success: success
                        .as_bool()
                        .ok_or(ThundersError::DeserializationFailure)?,
                })
            }

            DIFF => {
                let type_ = json
                    .get(TYPE)
                    .ok_or(ThundersError::DeserializationFailure)?;
                let id = json.get(ID).ok_or(ThundersError::DeserializationFailure)?;
                let finished = json
                    .get(FINISHED)
                    .ok_or(ThundersError::DeserializationFailure)?;

                let data = if let Some(value) = json.get(DATA) {
                    serde_json::to_vec(value).expect("Should always be deserializable")
                } else {
                    vec![]
                };

                Ok(OutputMessage::Diff {
                    type_: Cow::Owned(
                        type_
                            .as_str()
                            .ok_or(ThundersError::DeserializationFailure)?
                            .to_string(),
                    ),
                    id: Cow::Owned(
                        id.as_str()
                            .ok_or(ThundersError::DeserializationFailure)?
                            .to_string(),
                    ),
                    finished: finished
                        .as_bool()
                        .ok_or(ThundersError::DeserializationFailure)?,
                    data,
                })
            }

            GENERIC_ERROR => {
                let description = json
                    .get(DESCRIPTION)
                    .ok_or(ThundersError::DeserializationFailure)?;
                Ok(OutputMessage::GenericError {
                    description: Cow::Owned(
                        description
                            .as_str()
                            .ok_or(ThundersError::DeserializationFailure)?
                            .to_string(),
                    ),
                })
            }
            &_ => Err(ThundersError::DeserializationFailure),
        }
    }
}
