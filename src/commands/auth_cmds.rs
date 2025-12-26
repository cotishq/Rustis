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
        "GETUSER" => {
            if args.len() < 2 {
                return Value::Error("ERR wrong number of arguments for 'acl|getuser' command".into());
            }

            let username = match &args[1] {
                Value::BulkString(s) => s.as_str(),
                _ => return Value::Error("ERR invalid username".into()),
            };

            if username == "default" {
                Value::Array(vec![
                    Value::BulkString("flags".into()),
                    Value::Array(vec![
                        Value::BulkString("nopass".into()),
                    ]),
                    Value::BulkString("passwords".into()),
                    Value::Array(vec![])
                ])
            } else {
                Value::NullBulk
            }
        }
        _ => Value::Error(format!("ERR unknown subcommand '{}'", subcommand)),    
    }
}