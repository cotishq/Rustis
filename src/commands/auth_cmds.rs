use crate::resp::Value;
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::Mutex;

static ACL_USERS: Mutex<Option<HashMap<String, AclUser>>> = Mutex::new(None);

#[derive(Clone, Default)]
struct AclUser {
    passwords: Vec<String>,
}

fn get_or_init_users() -> std::sync::MutexGuard<'static, Option<HashMap<String, AclUser>>> {
    let mut guard = ACL_USERS.lock().unwrap();
    if guard.is_none() {
        let mut users = HashMap::new();
        users.insert("default".to_string(), AclUser::default());
        *guard = Some(users);
    }
    guard
}

fn sha256_hex(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

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
                Value::BulkString(s) => s.clone(),
                _ => return Value::Error("ERR invalid username".into()),
            };

            let guard = get_or_init_users();
            let users = guard.as_ref().unwrap();

            if let Some(user) = users.get(&username) {
                let mut flags = Vec::new();
                if user.passwords.is_empty() {
                    flags.push(Value::BulkString("nopass".into()));
                }

                let passwords: Vec<Value> = user.passwords
                    .iter()
                    .map(|p| Value::BulkString(p.clone()))
                    .collect();

                Value::Array(vec![
                    Value::BulkString("flags".into()),
                    Value::Array(flags),
                    Value::BulkString("passwords".into()),
                    Value::Array(passwords),
                ])
            } else {
                Value::NullBulk
            }
        }
        "SETUSER" => {
            if args.len() < 2 {
                return Value::Error("ERR wrong number of arguments for 'acl|setuser' command".into());
            }

            let username = match &args[1] {
                Value::BulkString(s) => s.clone(),
                _ => return Value::Error("ERR invalid username".into()),
            };

            let mut guard = get_or_init_users();
            let users = guard.as_mut().unwrap();

            let user = users.entry(username).or_insert_with(AclUser::default);

            for arg in &args[2..] {
                if let Value::BulkString(rule) = arg {
                    if let Some(password) = rule.strip_prefix('>') {
                        let hash = sha256_hex(password);
                        if !user.passwords.contains(&hash) {
                            user.passwords.push(hash);
                        }
                    }
                }
            }

            Value::SimpleString("OK".into())
        }
        _ => Value::Error(format!("ERR unknown subcommand '{}'", subcommand)),
    }
}

pub fn cmd_auth(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Error("ERR wrong number of arguments for 'auth' command".into());
    }

    let username = match &args[0] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::Error("ERR invalid username".into()),  
    };

    let password = match &args[1] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::Error("ERR invalid password".into()),  
    };

    let guard = get_or_init_users();
    let users = guard.as_ref().unwrap();

    if let Some(user) = users.get(&username) {
        let password_hash = sha256_hex(&password);
        if user.passwords.contains(&password_hash) {
            Value::SimpleString("OK".into())
        } else {
            Value::Error("WRONGPASS invalid username-password pair or user is disabled.".into())
        }
    } else {
        Value::Error("WRONGPASS invalid username-password pair or user is disabled.".into())
    }
}