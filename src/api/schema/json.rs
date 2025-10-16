use serde_json::{Value, value::RawValue};

use crate::api::{
    error::ThundersError,
    message::{InputMessage, OutputMessage},
    schema::{BorrowedSerialize, Deserialize, Schema, SchemaType, Serialize},
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

impl<'de, T> Deserialize<'de, Json> for T
where
    T: serde::Deserialize<'de>,
{
    fn deserialize(buf: &'de [u8]) -> Result<Self, ThundersError> {
        serde_json::from_slice(buf).map_err(|_| ThundersError::DeserializationFailure)
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
                            serde_json::from_slice::<Value>(options)
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
                            serde_json::from_slice::<Value>(data)
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

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, Visitor};
use std::borrow::Cow;

impl<'de> Deserialize<'de, Json> for InputMessage<'de> {
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
                        let k: Cow<'de2, str> =
                            <Cow<'de2, str> as serde::Deserialize>::deserialize(d)?;
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

                let mut method: Option<&'de2 str> = None;
                let mut corr: Option<&'de2 str> = None;
                let mut ty: Option<&'de2 str> = None;
                let mut id: Option<&'de2 str> = None;
                let mut p_id: Option<u64> = None;
                let mut options_bytes: Option<&'de2 [u8]> = None;
                let mut data_bytes: Option<&'de2 [u8]> = None;

                while let Some(f) = map.next_key_seed(FieldSeed)? {
                    match f {
                        Field::Method => method = Some(map.next_value()?),
                        Field::Corr => corr = Some(map.next_value()?),
                        Field::Type => ty = Some(map.next_value()?),
                        Field::Id => id = Some(map.next_value()?),
                        Field::PId => p_id = Some(map.next_value()?),
                        Field::Options => {
                            let raw: &RawValue = map.next_value()?;
                            options_bytes = Some(raw.get().as_bytes());
                        }
                        Field::Data => {
                            let raw: &RawValue = map.next_value()?;
                            data_bytes = Some(raw.get().as_bytes());
                        }
                        Field::Unknown => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let method = method.ok_or_else(|| de::Error::custom("missing `method`"))?;
                match method {
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
                            serde_json::from_slice::<Value>(data)
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

impl<'de> crate::api::schema::Deserialize<'de, Json> for OutputMessage<'de> {
    fn deserialize(buf: &'de [u8]) -> Result<Self, ThundersError> {
        let mut de = serde_json::Deserializer::from_slice(buf);

        struct Root;

        impl<'de2> Visitor<'de2> for Root {
            type Value = OutputMessage<'de2>;

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
                    Success,
                    Type,
                    Id,
                    Finished,
                    Data,
                    Description,
                    Unknown,
                }
                struct FieldSeed;
                impl<'de2> DeserializeSeed<'de2> for FieldSeed {
                    type Value = Field;
                    fn deserialize<D>(self, d: D) -> Result<Self::Value, D::Error>
                    where
                        D: Deserializer<'de2>,
                    {
                        let k: Cow<'de2, str> =
                            <Cow<'de2, str> as serde::Deserialize>::deserialize(d)?;
                        Ok(match &*k {
                            METHOD => Field::Method,
                            CORRELATION_ID => Field::Corr,
                            ID => Field::Id,
                            TYPE => Field::Type,
                            SUCCESS => Field::Success,
                            FINISHED => Field::Finished,
                            DATA => Field::Data,
                            DESCRIPTION => Field::Description,
                            _ => Field::Unknown,
                        })
                    }
                }

                let mut method: Option<&'de2 str> = None;
                let mut corr: Option<&'de2 str> = None;
                let mut ty: Option<&'de2 str> = None;
                let mut id: Option<&'de2 str> = None;
                let mut success: Option<bool> = None;
                let mut finished: Option<bool> = None;
                let mut description: Option<&'de2 str> = None;
                let mut data_bytes: Option<&'de2 [u8]> = None;

                while let Some(f) = map.next_key_seed(FieldSeed)? {
                    match f {
                        Field::Method => method = Some(map.next_value()?),
                        Field::Corr => corr = Some(map.next_value()?),
                        Field::Type => ty = Some(map.next_value()?),
                        Field::Id => id = Some(map.next_value()?),
                        Field::Success => success = Some(map.next_value()?),
                        Field::Finished => finished = Some(map.next_value()?),
                        Field::Description => description = Some(map.next_value()?),
                        Field::Data => {
                            let raw: &RawValue = map.next_value()?;
                            data_bytes = Some(raw.get().as_bytes());
                        }
                        Field::Unknown => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let method = method.ok_or_else(|| de::Error::custom("missing `method`"))?;
                match method {
                    CONNECT => {
                        let success = success
                            .ok_or_else(|| de::Error::custom("missing `success` for connect"))?;
                        let corr =
                            corr.ok_or_else(|| de::Error::custom("missing `correlation_id`"))?;
                        Ok(OutputMessage::Connect {
                            correlation_id: corr,
                            success,
                        })
                    }
                    CREATE => {
                        let success = success
                            .ok_or_else(|| de::Error::custom("missing `success` for connect"))?;
                        let corr =
                            corr.ok_or_else(|| de::Error::custom("missing `correlation_id`"))?;
                        Ok(OutputMessage::Create {
                            correlation_id: corr,
                            success,
                        })
                    }
                    JOIN => {
                        let success = success
                            .ok_or_else(|| de::Error::custom("missing `success` for connect"))?;
                        let corr =
                            corr.ok_or_else(|| de::Error::custom("missing `correlation_id`"))?;
                        Ok(OutputMessage::Join {
                            correlation_id: corr,
                            success,
                        })
                    }
                    DIFF => {
                        let finished = finished
                            .ok_or_else(|| de::Error::custom("missing `success` for connect"))?;
                        let ty = ty.ok_or_else(|| de::Error::custom("missing `type`"))?;
                        let id = id.ok_or_else(|| de::Error::custom("missing `id`"))?;
                        let data = data_bytes.unwrap_or_default();
                        Ok(OutputMessage::Diff {
                            type_: ty,
                            id,
                            finished,
                            data,
                        })
                    }
                    GENERIC_ERROR => {
                        let description =
                            description.ok_or_else(|| de::Error::custom("missing `type`"))?;
                        Ok(OutputMessage::GenericError { description })
                    }
                    _ => Err(de::Error::custom("unknown method")),
                }
            }
        }

        de.deserialize_map(Root)
            .map_err(|_| ThundersError::DeserializationFailure)
    }
}
