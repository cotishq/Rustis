use crate::resp::Value;
use crate::db::Db;
use super::string_cmds;
use super::list_cmds;

pub fn dispatch(cmd: &str, args: &[Value], db: &Db) -> Value {
    match cmd.to_lowercase().as_str() {
        "ping" => Value::SimpleString("PONG".into()),
        "echo" => args.first().cloned().unwrap_or(Value::SimpleString("".into())),
        "set" => string_cmds::cmd_set(db, args),
        "get" => string_cmds::cmd_get(db, args),
        "rpush" => list_cmds::cmd_rpush(db, args),
        "lrange" => list_cmds::cmd_lrange(db, args),
        c => Value::SimpleString(format!("ERR unknown command '{}'", c)),
    }
}
