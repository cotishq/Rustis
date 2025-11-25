use super::types::Value;

pub fn serialize(value: Value) -> String {
    value.serialize()
}
