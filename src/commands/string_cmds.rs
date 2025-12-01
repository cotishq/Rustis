use bytes::Bytes;
use crate::resp::Value;
use crate::db::Db;

fn unpack_bulk_str(value: &Value) -> Result<String, anyhow::Error> {
    match value {
        Value::BulkString(s) => Ok(s.clone()),
        _ => Err(anyhow::anyhow!("Expected bulk string")),
    }
}

pub fn cmd_set(db: &Db, args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::SimpleString("ERR wrong number of arguments for 'set' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::SimpleString("ERR invalid key".into()),
    };

    let value = match unpack_bulk_str(&args[1]) {
        Ok(v) => v,
        Err(_) => return Value::SimpleString("ERR invalid value".into()),
    };

    let expire = if args.len() >= 4 {
        if let Ok(flag) = unpack_bulk_str(&args[2]) {
            if flag.to_uppercase() == "PX" {
                if let Ok(ms_str) = unpack_bulk_str(&args[3]) {
                    if let Ok(ms) = ms_str.parse::<u64>() {
                        Some(tokio::time::Duration::from_millis(ms))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    db.set_string(key, Bytes::from(value), expire);
    Value::SimpleString("OK".into())
}

pub fn cmd_get(db: &Db, args: &[Value]) -> Value {
    if args.len() < 1 {
        return Value::SimpleString("ERR wrong number of arguments for 'get' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::SimpleString("ERR invalid key".into()),
    };

    match db.get_string(&key) {
        Some(b) => Value::BulkString(String::from_utf8(b.to_vec()).unwrap_or_default()),
        None => Value::BulkString("nil".into()),
    }
}

pub fn cmd_incr(db: &Db, args: &[Value]) -> Value {
    if args.len() != 1 {
        return Value::SimpleString("ERR wrong number of arguments for 'incr' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::SimpleString("ERR invalid key".into()),
    };

    match db.incr(&key) {
        Ok(val) => Value::Integer(val),
        Err(e) => Value::SimpleString(e.into()),
    }
}
