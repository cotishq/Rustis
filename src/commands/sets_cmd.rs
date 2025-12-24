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
