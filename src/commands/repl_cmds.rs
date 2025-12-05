use crate::resp::Value;
use crate::db::{Db, ServerRole};

fn unpack_bulk_str(value: &Value) -> Result<String, anyhow::Error> {
    match value {
        Value::BulkString(s) => Ok(s.clone()),
        _ => Err(anyhow::anyhow!("Expected bulk string")),
    }
}

pub fn cmd_info(db: &Db, args: &[Value]) -> Value {
    let section = if args.is_empty() {
        "replication".to_string()
    } else {
        match unpack_bulk_str(&args[0]) {
            Ok(s) => s.to_lowercase(),
            Err(_) => return Value::Error("ERR invalid section".into()),
        }
    };

    if section == "replication" {
        let role = match &db.config.role {
            ServerRole::Master => "master",
            ServerRole::Slave { .. } => "slave",
        };
        let info = format!("role:{}", role);
        Value::BulkString(info)
    } else {
        Value::Error(format!("ERR unsupported INFO section '{}'", section))
    }
}
