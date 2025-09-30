use serde_json::Value;

use crate::{
    core::hooks::DiffNotification,
    protocol::{InputMessage, ThundersError},
    schema::{Deserialize, Schema, SchemaType, Serialize},
};

#[derive(Default)]
pub struct Json {}

impl Schema for Json {
    fn schema_type() -> SchemaType {
        SchemaType::Text
    }
}

impl Deserialize<Json> for () {
    fn deserialize(_value: Vec<u8>) -> Result<Self, ThundersError> {
        Ok(())
    }
}
impl Serialize<Json> for () {
    fn serialize(self) -> Vec<u8> {
        vec![]
    }
}

impl Deserialize<Json> for InputMessage {
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError> {
        const METHOD: &str = "method";
        const CONNECT: &str = "connect";
        const ID: &str = "id";
        const CREATE: &str = "create";
        const TYPE: &str = "type";
        const OPTIONS: &str = "options";
        const DATA: &str = "data";
        const ACTION: &str = "action";
        const JOIN: &str = "join";

        let json: Value =
            serde_json::from_slice(value.as_slice()).map_err(|_| ThundersError::InvalidInput)?;

        let method = json.get(METHOD).ok_or(ThundersError::InvalidInput)?;
        match method.as_str().ok_or(ThundersError::InvalidInput)? {
            CONNECT => {
                let id = json.get(ID).ok_or(ThundersError::DeserializationFailure)?;
                Ok(InputMessage::Connect {
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
                let data: Vec<u8> = json
                    .get(DATA)
                    .and_then(|node| {
                        serde_json::to_string(node)
                            .map(|json| json.into_bytes())
                            .ok()
                    })
                    .ok_or(ThundersError::DeserializationFailure)?;

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
            &_ => Err(ThundersError::InvalidInput),
        }
    }
}

impl Serialize<Json> for ThundersError {
    fn serialize(self) -> Vec<u8> {
        match self {
            Self::RoomTypeNotFound => serde_json::from_str::<Value>(
                r#"
        {
            "type": "ROOM_TYPE_NOT_FOUND",
            "description": "The room type does not exists"
        }"#,
            )
            .expect("Should serialize successfully")
            .to_string()
            .into_bytes(),

            Self::RoomNotFound => serde_json::from_str::<Value>(
                r#"
        {
            "type": "ROOM_NOT_FOUND",
            "description": "The room does not exists"
        }"#,
            )
            .expect("Should serialize successfully")
            .to_string()
            .into_bytes(),
            Self::RoomAlreadyCreated => serde_json::from_str::<Value>(
                r#"
        {
            "type": "ROOM_ALREADY_CREATED",
            "description": "The room already exists"
        }"#,
            )
            .expect("Should serialize successfully")
            .to_string()
            .into_bytes(),

            Self::StartFailure => {
                vec![]
            }
            Self::MessageNotConnected => serde_json::from_str::<Value>(
                r#"
        {
            "type": "NOT_CONNECTED",
            "description": "Received message without player be connected"
        }"#,
            )
            .expect("Should serialize successfully")
            .to_string()
            .into_bytes(),

            Self::ConnectionFailure => serde_json::from_str::<Value>(
                r#"
        {
            "type": "CONNECTION_FAILURE",
            "description": ""
        }"#,
            )
            .expect("Should serialize successfully")
            .to_string()
            .into_bytes(),
            Self::InvalidInput => serde_json::from_str::<Value>(
                r#"
        {
            "type": "INVALID_INPUT",
            "description": ""
        }"#,
            )
            .expect("Should serialize successfully")
            .to_string()
            .into_bytes(),
            Self::DeserializationFailure => serde_json::from_str::<Value>(
                r#"
        {
            "type": "INVALID_INPUT",
            "description": ""
        }"#,
            )
            .expect("Should serialize successfully")
            .to_string()
            .into_bytes(),
        }
    }
}

impl Serialize<Json> for DiffNotification<'_> {
    fn serialize(mut self) -> Vec<u8> {
        let json;

        if self.finished {
            json = format!(
                r#"
            {{
            "type" : "{}",
            "id": "{}",
            "finished": true,
            "data": {{}}}}                
            "#,
                self.type_, self.id
            );
        } else {
            json = format!(
                r#"
            {{
            "type" : "{}",
            "id": "{}",
            "data":               
            "#,
                self.type_, self.id
            );
        }

        let mut raw_message = json.into_bytes();

        if !self.finished {
            raw_message.append(&mut self.data);
            raw_message.push(b'}');
        }
        raw_message
    }
}
