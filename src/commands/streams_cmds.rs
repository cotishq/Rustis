

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

    let auto_seq = id_str.ends_with("-*");
    let full_auto = id_str == "*";

    if full_auto{
        use std::time::{SystemTime , UNIX_EPOCH};

        let ms_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let seq_now = db.next_sequence_for_ms(&key, ms_now);

        let new_id_str = format!("{}-{}" , ms_now , seq_now);

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

        db.xadd(key, new_id_str.clone(), fields);
        return Value::BulkString(new_id_str);
    }

    let ( ms, mut seq) = if auto_seq {
    // Extract the ms part
    let parts: Vec<&str> = id_str.split('-').collect();
    if parts.len() != 2 {
        return Value::SimpleString("ERR invalid ID format".into());
    }
    let ms_val = parts[0].parse::<i64>().unwrap_or(-1);

    if ms_val < 0 {
        return Value::SimpleString("ERR invalid time part".into());
    }

    (ms_val, -1) // seq = -1 means auto-generate
    } else {
        match parse_id(&id_str) {
            Some(t) => t,
            None => return Value::SimpleString("Err invalid ID format".into()),
        }
    };

    if auto_seq{
        seq = db.next_sequence_for_ms(&key, ms);
    }

   if !auto_seq {
    if ms == 0 && seq == 0 {
        return Value::SimpleString(
            "Err The ID specified in XADD must be greater than 0-0".into()
        );
    }

    if let Some((last_ms , last_seq)) = db.get_last_stream_id(&key) {
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

    let new_id_str = format!("{}-{}", ms, seq);
    db.xadd(key, new_id_str.clone(), fields);
    Value::BulkString(new_id_str)

}

fn parse_range_id(id: &str, is_start: bool) -> Option<(i64, i64)> {
    let parts: Vec<&str> = id.split('-').collect();
    match parts.len() {
        1 => {
            let ms = parts[0].parse::<i64>().ok()?;
            let seq = if is_start { 0 } else { i64::MAX };
            Some((ms, seq))
        }
        2 => {
            let ms = parts[0].parse::<i64>().ok()?;
            let seq = parts[1].parse::<i64>().ok()?;
            Some((ms, seq))
        }
        _ => None,
    }
}

pub fn cmd_xrange(db: &Db, args: &[Value]) -> Value {
    if args.len() < 3 {
        return Value::SimpleString("ERR wrong number of arguments for 'xrange' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::SimpleString("ERR invalid key".into()),
    };

    let start_str = match unpack_bulk_str(&args[1]) {
        Ok(s) => s,
        Err(_) => return Value::SimpleString("ERR invalid start ID".into()),
    };

    let end_str = match unpack_bulk_str(&args[2]) {
        Ok(s) => s,
        Err(_) => return Value::SimpleString("ERR invalid end ID".into()),
    };

    let start = match parse_range_id(&start_str, true) {
        Some(id) => id,
        None => return Value::SimpleString("ERR invalid start ID".into()),
    };

    let end = match parse_range_id(&end_str, false) {
        Some(id) => id,
        None => return Value::SimpleString("ERR invalid end ID".into()),
    };

    let entries = db.xrange(&key, start, end);

    let result: Vec<Value> = entries
        .iter()
        .map(|entry| {
            let id = Value::BulkString(entry.id.clone());
            let fields: Vec<Value> = entry
                .fields
                .iter()
                .flat_map(|(k, v)| vec![Value::BulkString(k.clone()), Value::BulkString(v.clone())])
                .collect();
            Value::Array(vec![id, Value::Array(fields)])
        })
        .collect();

    Value::Array(result)
}
