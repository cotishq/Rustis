use crate::resp::Value;
use crate::db::Db;

fn unpack_bulk_str(value: &Value) -> Result<String, anyhow::Error> {
    match value {
        Value::BulkString(s) => Ok(s.clone()),
        _ => Err(anyhow::anyhow!("Expected bulk string")),
    }
}

pub fn cmd_config(db: &Db, args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Error("ERR wrong number of arguments for 'config' command".into());
    }

    let subcommand = match unpack_bulk_str(&args[0]) {
        Ok(s) => s.to_ascii_uppercase(),
        Err(_) => return Value::Error("ERR invalid argument".into()),
    };

    match subcommand.as_str() {
        "GET" => cmd_config_get(db, &args[1..]),
        _ => Value::Error(format!("ERR Unknown subcommand or wrong number of arguments for '{}'", subcommand)),
    }
}

fn cmd_config_get(db: &Db, args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Error("ERR wrong number of arguments for 'config|get' command".into());
    }

    let param = match unpack_bulk_str(&args[0]) {
        Ok(s) => s.to_ascii_lowercase(),
        Err(_) => return Value::Error("ERR invalid argument".into()),
    };

    match param.as_str() {
        "dir" => {
            let value = db.config.dir.clone().unwrap_or_default();
            Value::Array(vec![
                Value::BulkString("dir".into()),
                Value::BulkString(value),
            ])
        }
        "dbfilename" => {
            let value = db.config.dbfilename.clone().unwrap_or_default();
            Value::Array(vec![
                Value::BulkString("dbfilename".into()),
                Value::BulkString(value),
            ])
        }
        _ => Value::Array(vec![]),
    }
}

pub fn cmd_keys(db: &Db, args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Error("ERR wrong number of arguments for 'keys' command".into());
    }

    let pattern = match unpack_bulk_str(&args[0]) {
        Ok(p) => p,
        Err(_) => return Value::Error("ERR invalid pattern".into()),
    };

    // Currently only supports "*" pattern
    if pattern != "*" {
        return Value::Error("ERR only '*' pattern is supported".into());
    }

    let keys = db.keys(&pattern);
    Value::Array(keys.into_iter().map(Value::BulkString).collect())
}