
use crate::resp::Value;
use crate::db::Db;

fn unpack_bulk_str(value: &Value) -> Result<String, anyhow::Error> {
    match value {
        Value::BulkString(s) => Ok(s.clone()),
        _ => Err(anyhow::anyhow!("Expected bulk string")),
    }
}

pub fn parse_id(id: &str) -> Option<(i64, i64)> {
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 2 {
        return None;
    }
    let ms = parts[0].parse::<i64>().ok()?;
    let seq = parts[1].parse::<i64>().ok()?;
    Some((ms, seq))
}


pub fn cmd_type(db: &Db , args:&[Value]) -> Value{
    if args.len() != 1{
        return Value::SimpleString("Err wrong number of arguments for 'type' command".into() );
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::SimpleString("Err invalid key".into()),
    };

    let t = db.get_type(&key);
    Value::BulkString(t)
}

pub async fn cmd_xadd(db: &Db, args: &[Value]) -> Value {

    if args.len() < 4 || (args.len() - 2) % 2 != 0 {
        return Value::SimpleString("ERR wrong number of arguments".into());
    }

    let key = match &args[0] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::SimpleString("ERR invalid key".into()),
    };

    let id_str = match &args[1] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::SimpleString("ERR invalid id".into()),
    };

    let (ms , seq) = match parse_id(&id_str) {
        Some(t) => t,
        None => return Value::SimpleString("Err invalid ID format".into()),
        
    };

    if ms == 0 && seq == 0{
        return Value::SimpleString("Err The ID specified in XADD must be greater than 0-0".into());
    }

    let last_id_opt = db.get_last_stream_id(&key);

    if let Some((last_ms , last_seq)) = last_id_opt{
        let invalid = ms < last_ms || (ms == last_ms && seq <= last_seq);

        if invalid {
            return Value::SimpleString(
                "Err The ID specified in XADD is equal or smaller than the target stream top item".into()
            );
        }
    } else {
        if ms == 0 && seq < 1 {
            return Value::SimpleString("Err The ID specified in XADD must be greater than 0-0".into());
        }
    }

    let mut fields = Vec::new();

    for i in (2..args.len()).step_by(2) {
        let field = match &args[i] {
            Value::BulkString(s) => s.clone(),
            _ => return Value::SimpleString("ERR invalid field".into()),
        };

        let value = match &args[i + 1] {
            Value::BulkString(s) => s.clone(),
            _ => return Value::SimpleString("ERR invalid value".into()),
        };

        fields.push((field, value));
    }

    let new_id = db.xadd(key, id_str.clone(), fields);
    Value::BulkString(new_id)
}
