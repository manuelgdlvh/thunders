use crate::api::error::ThundersError;

pub mod json;

pub trait Schema {
    fn schema_type() -> SchemaType;
}

pub enum SchemaType {
    Text,
    Binary,
}

pub trait Deserialize<S>
where
    S: Schema,
    Self: Sized,
{
    fn deserialize(value: Vec<u8>) -> Result<Self, ThundersError>;
}

pub trait Serialize<S>
where
    S: Schema,
    Self: Sized,
{
    fn serialize(self) -> Vec<u8>;
}
