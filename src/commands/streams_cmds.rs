
use crate::resp::Value;
use crate::db::Db;

fn unpack_bulk_str(value: &Value) -> Result<String, anyhow::Error> {
    match value {
        Value::BulkString(s) => Ok(s.clone()),
        _ => Err(anyhow::anyhow!("Expected bulk string")),
    }
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

    let id = match &args[1] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::SimpleString("ERR invalid id".into()),
    };

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

    let new_id = db.xadd(key, id.clone(), fields);
    Value::BulkString(new_id)
}
