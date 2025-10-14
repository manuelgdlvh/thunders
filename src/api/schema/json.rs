use serde_json::Value;

use crate::api::{
    error::ThundersError,
    message::{InputMessage, OutputMessage},
    schema::{BorrowedDeserialize, BorrowedSerialize, Schema, SchemaType, Serialize},
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

impl<T> BorrowedSerialize<Json> for T
where
    T: serde::Serialize,
{
    fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Should always be serializable")
    }
}

impl<T> crate::api::schema::Deserialize<Json> for T
where
    for<'de> T: serde::Deserialize<'de>,
{
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError> {
        serde_json::from_slice(&value).map_err(|_| ThundersError::DeserializationFailure)
    }
}

impl Serialize<Json> for InputMessage<'_> {
    fn serialize(self) -> Vec<u8> {
        match self {
            Self::Connect { correlation_id, id } => serde_json::json!({
                "method": "connect",
                "correlation_id": correlation_id,
                "p_id": id
            }),
            Self::Create {
                correlation_id,
                type_,
                id,
                options,
            } => {
                let mut json_node = serde_json::json!({
                    "method": "create",
                    "correlation_id": correlation_id,
                    "type": type_,
                    "id": id
                });

                if let Some(options) = options {
                    json_node
                        .as_object_mut()
                        .expect("Should always be a object")
                        .insert(
                            OPTIONS.to_string(),
                            serde_json::from_slice::<Value>(options.as_slice())
                                .expect("Should always be serializable"),
                        );
                }

                json_node
            }
            Self::Join {
                correlation_id,
                type_,
                id,
            } => serde_json::json!({
                "method": "join",
                "type": type_,
                "correlation_id": correlation_id,
                "id": id
            }),
            Self::Action { type_, id, data } => {
                let mut json_node = serde_json::json!({
                    "method": "action",
                    "type": type_,
                    "id": id
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

use serde::de::{self, Deserialize, DeserializeSeed, Deserializer, MapAccess, Visitor};
use std::borrow::Cow;

impl<'de> BorrowedDeserialize<'de, Json> for InputMessage<'de>
where
    Json: Schema,
{
    fn deserialize(buf: &'de [u8]) -> Result<Self, ThundersError> {
        let mut de = serde_json::Deserializer::from_slice(buf);

        struct Root;

        impl<'de2> Visitor<'de2> for Root {
            type Value = InputMessage<'de2>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("flat JSON {method, correlation_id, id, p_id, type, options?, data?}")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de2>,
            {
                enum Field {
                    Method,
                    Corr,
                    Id,
                    PId,
                    Type,
                    Options,
                    Data,
                    Unknown,
                }
                struct FieldSeed;
                impl<'de2> DeserializeSeed<'de2> for FieldSeed {
                    type Value = Field;
                    fn deserialize<D>(self, d: D) -> Result<Self::Value, D::Error>
                    where
                        D: Deserializer<'de2>,
                    {
                        let k: Cow<'de2, str> = Cow::deserialize(d)?;
                        Ok(match &*k {
                            METHOD => Field::Method,
                            CORRELATION_ID => Field::Corr,
                            ID => Field::Id,
                            PLAYER_ID => Field::PId,
                            TYPE => Field::Type,
                            OPTIONS => Field::Options,
                            DATA => Field::Data,
                            _ => Field::Unknown,
                        })
                    }
                }

                let mut method: Option<Cow<'de2, str>> = None;
                let mut corr: Option<Cow<'de2, str>> = None;
                let mut ty: Option<Cow<'de2, str>> = None;
                let mut id: Option<Cow<'de2, str>> = None;
                let mut p_id: Option<u64> = None;
                let mut options_bytes: Option<Vec<u8>> = None;
                let mut data_bytes: Option<Vec<u8>> = None;

                while let Some(f) = map.next_key_seed(FieldSeed)? {
                    match f {
                        Field::Method => method = Some(map.next_value()?),
                        Field::Corr => corr = Some(map.next_value()?),
                        Field::Type => ty = Some(map.next_value()?),
                        Field::Id => id = Some(map.next_value()?),
                        Field::PId => p_id = Some(map.next_value()?),
                        Field::Options => {
                            let v: serde_json::Value = map.next_value()?;
                            options_bytes = Some(serde_json::to_vec(&v).unwrap_or_default());
                        }
                        Field::Data => {
                            let v: serde_json::Value = map.next_value()?;
                            data_bytes = Some(serde_json::to_vec(&v).unwrap_or_default());
                        }
                        Field::Unknown => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let method = method.ok_or_else(|| de::Error::custom("missing `method`"))?;
                match &*method {
                    CONNECT => {
                        let id_num =
                            p_id.ok_or_else(|| de::Error::custom("missing `p_id` for connect"))?;
                        let corr =
                            corr.ok_or_else(|| de::Error::custom("missing `correlation_id`"))?;
                        Ok(InputMessage::Connect {
                            correlation_id: corr,
                            id: id_num,
                        })
                    }
                    CREATE => {
                        let corr =
                            corr.ok_or_else(|| de::Error::custom("missing `correlation_id`"))?;
                        let ty = ty.ok_or_else(|| de::Error::custom("missing `type`"))?;
                        let id = id.ok_or_else(|| de::Error::custom("missing `id`"))?;
                        Ok(InputMessage::Create {
                            correlation_id: corr,
                            type_: ty,
                            id,
                            options: options_bytes,
                        })
                    }
                    JOIN => {
                        let corr =
                            corr.ok_or_else(|| de::Error::custom("missing `correlation_id`"))?;
                        let ty = ty.ok_or_else(|| de::Error::custom("missing `type`"))?;
                        let id = id.ok_or_else(|| de::Error::custom("missing `id`"))?;
                        Ok(InputMessage::Join {
                            correlation_id: corr,
                            type_: ty,
                            id,
                        })
                    }
                    ACTION => {
                        let ty = ty.ok_or_else(|| de::Error::custom("missing `type`"))?;
                        let id = id.ok_or_else(|| de::Error::custom("missing `id`"))?;
                        let data = data_bytes.unwrap_or_default();
                        Ok(InputMessage::Action {
                            type_: ty,
                            id,
                            data,
                        })
                    }
                    _ => Err(de::Error::custom("unknown method")),
                }
            }
        }

        de.deserialize_map(Root)
            .map_err(|_| ThundersError::DeserializationFailure)
    }
}

// Output

const METHOD: &str = "method";
const CORRELATION_ID: &str = "correlation_id";

const CONNECT: &str = "connect";

const JOIN: &str = "join";
const CREATE: &str = "create";
const GENERIC_ERROR: &str = "generic_error";
const DIFF: &str = "diff";
const ACTION: &str = "action";

const DATA: &str = "data";

const OPTIONS: &str = "options";
const FINISHED: &str = "finished";
const TYPE: &str = "type";
const ID: &str = "id";

const PLAYER_ID: &str = "p_id";
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
            OutputMessage::Create {
                correlation_id,
                success,
            } => serde_json::json!({
                METHOD: CREATE,
                CORRELATION_ID: correlation_id,
                SUCCESS: success
            }),
            OutputMessage::Join {
                correlation_id,
                success,
            } => serde_json::json!({
                METHOD: JOIN,
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

impl<'a> crate::api::schema::Deserialize<Json> for OutputMessage<'a> {
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
            JOIN => {
                let success = json
                    .get(SUCCESS)
                    .ok_or(ThundersError::DeserializationFailure)?;
                let correlation_id = json
                    .get(CORRELATION_ID)
                    .ok_or(ThundersError::DeserializationFailure)?;
                Ok(OutputMessage::Join {
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
            CREATE => {
                let success = json
                    .get(SUCCESS)
                    .ok_or(ThundersError::DeserializationFailure)?;
                let correlation_id = json
                    .get(CORRELATION_ID)
                    .ok_or(ThundersError::DeserializationFailure)?;
                Ok(OutputMessage::Create {
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
