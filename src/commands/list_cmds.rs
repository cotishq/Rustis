use bytes::Bytes;
use crate::resp::Value;
use crate::db::Db;

fn unpack_bulk_str(value: &Value) -> Result<String, anyhow::Error> {
    match value {
        Value::BulkString(s) => Ok(s.clone()),
        _ => Err(anyhow::anyhow!("Expected bulk string")),
    }
}

pub fn cmd_rpush(db: &Db, args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::SimpleString("ERR wrong number of arguments for 'rpush' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::SimpleString("ERR invalid key".into()),
    };

    let values: Vec<Bytes> = args
        .iter()
        .skip(1)
        .filter_map(|v| unpack_bulk_str(v).ok())
        .map(|s| Bytes::from(s))
        .collect();

    let len = db.rpush(key, values);
    Value::Integer(len as i64)
}
pub fn cmd_lpush(db: &Db, args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::SimpleString("ERR wrong number of arguments for 'rpush' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::SimpleString("ERR invalid key".into()),
    };

    let values: Vec<Bytes> = args
        .iter()
        .skip(1)
        .filter_map(|v| unpack_bulk_str(v).ok())
        .map(Bytes::from)
        .collect();

    let len = db.lpush(key, values);
    Value::Integer(len as i64)
}



pub fn cmd_lrange(db: &Db, args: &[Value]) -> Value {
    if args.len() < 3 {
        return Value::SimpleString("ERR wrong number of arguments for 'lrange' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::SimpleString("ERR invalid key".into()),
    };

    let start: i64 = unpack_bulk_str(&args[1])
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let end: i64 = unpack_bulk_str(&args[2])
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(-1);

    let elements = db.lrange(&key, start, end);
    let values: Vec<Value> = elements
        .into_iter()
        .map(|b| Value::BulkString(String::from_utf8(b.to_vec()).unwrap_or_default()))
        .collect();

    Value::Array(values)
}
