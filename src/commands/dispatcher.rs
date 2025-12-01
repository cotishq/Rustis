use crate::resp::Value;
use crate::db::Db;
use super::string_cmds;
use super::list_cmds;
use super::streams_cmds;

pub async  fn dispatch(cmd: &str, args: &[Value], db: &Db) -> Value {
    match cmd.to_lowercase().as_str() {
        "ping" => Value::SimpleString("PONG".into()),
        "echo" => args.first().cloned().unwrap_or(Value::SimpleString("".into())),
        "set" => string_cmds::cmd_set(db, args),
        "get" => string_cmds::cmd_get(db, args),
        "rpush" => list_cmds::cmd_rpush(db, args),
        "lpush" => list_cmds::cmd_lpush(db, args),
        "llen" => list_cmds::cmd_llen(db, args),
        "lrange" => list_cmds::cmd_lrange(db, args),
        "lpop" => list_cmds::cmd_lpop(db, args),
        "type" => streams_cmds::cmd_type(db, args),
        "xadd" => streams_cmds::cmd_xadd(db, args).await,
        "xrange" => streams_cmds::cmd_xrange(db, args),
        "xread" => streams_cmds::cmd_xread(db, args),
        c => Value::SimpleString(format!("ERR unknown command '{}'", c)),
    }
}
