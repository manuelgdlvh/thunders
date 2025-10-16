use crate::api::error::ThundersError;

#[cfg(feature = "json")]
pub mod json;

pub trait Schema {
    fn schema_type() -> SchemaType;
}

pub enum SchemaType {
    Text,
    Binary,
}

pub trait Deserialize<'de, S>
where
    S: Schema,
    Self: Sized,
{
    fn deserialize(buf: &'de [u8]) -> Result<Self, ThundersError>;
}

pub trait Serialize<S>
where
    S: Schema,
    Self: Sized,
{
    fn serialize(self) -> Vec<u8>;
}

pub trait BorrowedSerialize<S>
where
    S: Schema,
    Self: Sized,
{
    fn serialize(&self) -> Vec<u8>;
}
