use crate::resp::Value;
use crate::db::Db;

fn unpack_bulk_str(value: &Value) -> Result<String, anyhow::Error> {
    match value {
        Value::BulkString(s) => Ok(s.clone()),
        _ => Err(anyhow::anyhow!("Expected bulk string")),
    }
}

pub fn cmd_zadd(db: &Db, args: &[Value]) -> Value {
    if args.len() < 3 {
        return Value::Error("ERR wrong number of arguments for 'zadd' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    let score_str = match unpack_bulk_str(&args[1]) {
        Ok(s) => s,
        Err(_) => return Value::Error("ERR invalid score".into()),
    };

    let score: f64 = match score_str.parse() {
        Ok(s) => s,
        Err(_) => return Value::Error("ERR value is not a valid float".into()),
    };

    let member = match unpack_bulk_str(&args[2]) {
        Ok(m) => m,
        Err(_) => return Value::Error("ERR invalid member".into()),
    };

    let added = db.zadd(key, score, member);
    Value::Integer(added as i64)
}

pub fn cmd_zrank(db: &Db, args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Error("ERR wrong number of arguments for 'zrank' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    let member = match unpack_bulk_str(&args[1]) {
        Ok(m) => m,
        Err(_) => return Value::Error("ERR invalid member".into()),
    };

    match db.zrank(&key, &member) {
        Some(rank) => Value::Integer(rank as i64),
        None => Value::NullBulk,
    }
}

pub fn cmd_zrange(db: &Db, args: &[Value]) -> Value {
    if args.len() < 3 {
        return Value::Error("ERR wrong number of arguments for 'zrange' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    let start_str = match unpack_bulk_str(&args[1]) {
        Ok(s) => s,
        Err(_) => return Value::Error("ERR invalid start index".into()),
    };

    let start: i64 = match start_str.parse() {
        Ok(s) => s,
        Err(_) => return Value::Error("ERR value is not an integer or out of range".into()),
    };

    let stop_str = match unpack_bulk_str(&args[2]) {
        Ok(s) => s,
        Err(_) => return Value::Error("ERR invalid stop index".into()),
    };

    let stop: i64 = match stop_str.parse() {
        Ok(s) => s,
        Err(_) => return Value::Error("ERR value is not an integer or out of range".into()),
    };

    let members = db.zrange(&key, start, stop);
    Value::Array(members.into_iter().map(Value::BulkString).collect())
}

pub fn cmd_zcard(db: &Db, args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Error("ERR wrong number of arguments for 'zcard' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    Value::Integer(db.zcard(&key) as i64)
}

pub fn cmd_zscore(db: &Db, args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Error("ERR wrong number of arguments for 'zscore' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    let member = match unpack_bulk_str(&args[1]) {
        Ok(m) => m,
        Err(_) => return Value::Error("ERR invalid member".into()),
    };

    match db.zscore(&key, &member) {
        Some(score) => Value::BulkString(score.to_string()),
        None => Value::NullBulk,
    }
}
