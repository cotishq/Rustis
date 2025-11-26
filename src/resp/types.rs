#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    SimpleString(String),
    BulkString(String),
    Integer(i64),
    Array(Vec<Value>),
    NullBulk,
}

impl Value {
    /// Convert the value to a RESP-formatted string for transmission
    pub fn serialize(&self) -> String {
        match self {
            Value::SimpleString(s) => format!("+{}\r\n", s),

            Value::BulkString(s) => {
                format!("${}\r\n{}\r\n", s.len(), s)
            }

            Value::Integer(n) => format!(":{}\r\n", n),

            Value::Array(arr) => {
                let mut out = format!("*{}\r\n", arr.len());

                for v in arr {
                    out.push_str(&v.serialize());
                }

                out
            },

            Value::NullBulk => "$-1\r\n".to_string(),
        }
    }
}
