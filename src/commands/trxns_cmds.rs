use crate::resp::Value;
use crate::db::Db;

pub fn cmd_multi(_db: &Db, _args: &[Value]) -> Value {
    Value::SimpleString("OK".into())
}

pub fn cmd_exec(_db : &Db , _args: &[Value]) -> Value{
    Value::Error("ERR EXEC without MULTI".into())
}

pub fn cmd_discard(_db: &Db, _args: &[Value]) -> Value {
    Value::Error("ERR DISCARD without MULTI".into())
}
