use std::collections::HashSet;

use crate::resp::Value;

pub fn cmd_subscribe(args: &[Value], subscribed_channels: &mut HashSet<String>) -> Value {
    if args.is_empty() {
        return Value::Error("ERR wrong number of arguments for 'subscribe' command".into());
    }

    let channel = match &args[0] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::Error("ERR invalid channel name".into()),
    };

    subscribed_channels.insert(channel.clone());

    Value::Array(vec![
        Value::BulkString("subscribe".into()),
        Value::BulkString(channel),
        Value::Integer(subscribed_channels.len() as i64),
    ])
}
