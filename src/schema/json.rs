use std::io::Error;

use serde_json::Value;

use crate::{
    protocol::InputMessage,
    schema::{DeSerialize, Schema, SchemaType},
};

#[derive(Default)]
pub struct Json {}

impl Schema for Json {
    fn schema_type() -> SchemaType {
        SchemaType::Text
    }
}

impl DeSerialize<Json> for () {
    fn deserialize(_value: Vec<u8>) -> Result<Self, Error> {
        Ok(())
    }

    fn serialize(self) -> Vec<u8> {
        vec![]
    }
}

impl DeSerialize<Json> for InputMessage {
    fn deserialize(value: Vec<u8>) -> Result<Self, Error> {
        let json: Value = serde_json::from_slice(value.as_slice())
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;

        let method = json
            .get("method")
            .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;

        match method
            .as_str()
            .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?
        {
            "connect" => {
                let id = json
                    .get("id")
                    .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;
                Ok(InputMessage::Connect {
                    id: id
                        .as_u64()
                        .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?,
                })
            }
            "create" => {
                let type_ = json
                    .get("type")
                    .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;

                let id = json
                    .get("id")
                    .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;

                let options: Option<Vec<u8>> = json.get("options").and_then(|node| {
                    serde_json::to_string(node)
                        .map(|json| json.into_bytes())
                        .ok()
                });

                Ok(InputMessage::Create {
                    type_: type_
                        .as_str()
                        .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?
                        .to_string(),
                    id: id
                        .as_str()
                        .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?
                        .to_string(),
                    options,
                })
            }

            "join" => {
                let type_ = json
                    .get("type")
                    .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;

                let id = json
                    .get("id")
                    .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;

                Ok(InputMessage::Join {
                    type_: type_
                        .as_str()
                        .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?
                        .to_string(),
                    id: id
                        .as_str()
                        .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?
                        .to_string(),
                })
            }
            "action" => {
                let type_ = json
                    .get("type")
                    .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;

                let id = json
                    .get("id")
                    .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;

                let data: Vec<u8> = json
                    .get("data")
                    .and_then(|node| {
                        serde_json::to_string(node)
                            .map(|json| json.into_bytes())
                            .ok()
                    })
                    .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?;

                Ok(InputMessage::Action {
                    type_: type_
                        .as_str()
                        .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?
                        .to_string(),
                    id: id
                        .as_str()
                        .ok_or(Error::new(std::io::ErrorKind::InvalidInput, ""))?
                        .to_string(),
                    data,
                })
            }
            &_ => Err(Error::new(std::io::ErrorKind::InvalidInput, "")),
        }
    }

    fn serialize(self) -> Vec<u8> {
        vec![]
    }
}
