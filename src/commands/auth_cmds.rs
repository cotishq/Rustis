use crate::resp::Value;

pub fn cmd_acl(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Error("ERR wrong number of arguments for 'acl' command".into());
    }

    let subcommand = match &args[0] {
        Value::BulkString(s) => s.to_uppercase(),
        _ => return Value::Error("ERR invalid subcommand".into()),
    };

    match subcommand.as_str() {
        "WHOAMI" => Value::BulkString("default".into()),
        _ => Value::Error(format!("ERR unknown subcommand '{}'", subcommand))
        
    }
}