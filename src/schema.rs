use std::io::Error;

pub mod json;

pub trait Schema {
    fn schema_type() -> SchemaType;
}

pub enum SchemaType {
    Text,
    Binary,
}

pub trait DeSerialize<S>
where
    S: Schema,
    Self: Sized,
{
    fn deserialize(value: Vec<u8>) -> Result<Self, Error>;

    fn serialize(self) -> Vec<u8>;
}
