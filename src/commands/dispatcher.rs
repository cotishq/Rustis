use crate::resp::Value;
use crate::db::Db;
use crate::client::ClientState;
use super::string_cmds;
use super::list_cmds;
use super::streams_cmds;
use super::repl_cmds;
use super::persistence_cmds;
use super::pubsub_cmds;
use super::sets_cmd;

fn is_allowed_in_subscribed_mode(cmd: &str) -> bool {
    matches!(
        cmd,
        "subscribe" | "unsubscribe" | "psubscribe" | "punsubscribe" | "ping" | "quit" | "reset"
    )
}

pub async fn dispatch(cmd: &str, args: &[Value], db: &Db, client: &mut ClientState) -> Value {
    let cmd_lower = cmd.to_lowercase();

    if client.is_subscribed() && !is_allowed_in_subscribed_mode(&cmd_lower) {
        return Value::Error(format!(
            "ERR Can't execute '{}': only (P|S)SUBSCRIBE / (P|S)UNSUBSCRIBE / PING / QUIT / RESET are allowed in this context",
            cmd_lower
        ));
    }

    match cmd_lower.as_str() {
        "ping" => {
            if client.is_subscribed() {
                // In subscribed mode, return ["pong", ""] as array
                Value::Array(vec![
                    Value::BulkString("pong".into()),
                    Value::BulkString("".into()),
                ])
            } else {
                Value::SimpleString("PONG".into())
            }
        }
        "echo" => args.first().cloned().unwrap_or(Value::SimpleString("".into())),
        "set" => string_cmds::cmd_set(db, args),
        "get" => string_cmds::cmd_get(db, args),
        "incr" => string_cmds::cmd_incr(db, args),
        "rpush" => list_cmds::cmd_rpush(db, args),
        "lpush" => list_cmds::cmd_lpush(db, args),
        "llen" => list_cmds::cmd_llen(db, args),
        "lrange" => list_cmds::cmd_lrange(db, args),
        "lpop" => list_cmds::cmd_lpop(db, args),
        "type" => streams_cmds::cmd_type(db, args),
        "xadd" => streams_cmds::cmd_xadd(db, args).await,
        "xrange" => streams_cmds::cmd_xrange(db, args),
        "xread" => streams_cmds::cmd_xread(db, args),
        "multi" => {
            client.in_transaction = true;
            client.queued_commands.clear();
            Value::SimpleString("OK".into())
        }
        "exec" => {
            if client.in_transaction {
                client.in_transaction = false;
                let mut results = Vec::new();
                let commands: Vec<_> = client.queued_commands.drain(..).collect();
                for (cmd, cmd_args) in commands {
                    let result = dispatch_inner(&cmd, &cmd_args, db).await;
                    results.push(result);
                }
                Value::Array(results)
            } else {
                Value::Error("ERR EXEC without MULTI".into())
            }
        }
        "discard" => {
            if client.in_transaction {
                client.in_transaction = false;
                client.queued_commands.clear();
                Value::SimpleString("OK".into())
            } else {
                Value::Error("ERR DISCARD without MULTI".into())
            }
        }
        "info" => repl_cmds::cmd_info(db, args),
        "replconf" => repl_cmds::cmd_replconf(db, args),
        "psync" => repl_cmds::cmd_psync(db, args),
        "config" => persistence_cmds::cmd_config(db, args),
        "subscribe" => pubsub_cmds::cmd_subscribe(args, &mut client.subscribed_channels, &client.pubsub_tx, db),
        "publish" => pubsub_cmds::cmd_publish(args, db),
        "zadd" => sets_cmd::cmd_zadd(db, args),
        "zrank" => sets_cmd::cmd_zrank(db, args),
        "zrange" => sets_cmd::cmd_zrange(db, args),
        "zcard" => sets_cmd::cmd_zcard(db, args),
        "zscore" => sets_cmd::cmd_zscore(db, args),
        "zrem" => sets_cmd::cmd_zrem(db, args),
        c => Value::Error(format!("ERR unknown command '{}'", c)),
    }
}

async fn dispatch_inner(cmd: &str, args: &[Value], db: &Db) -> Value {
    match cmd.to_lowercase().as_str() {
        "ping" => Value::SimpleString("PONG".into()),
        "echo" => args.first().cloned().unwrap_or(Value::SimpleString("".into())),
        "set" => string_cmds::cmd_set(db, args),
        "get" => string_cmds::cmd_get(db, args),
        "incr" => string_cmds::cmd_incr(db, args),
        "rpush" => list_cmds::cmd_rpush(db, args),
        "lpush" => list_cmds::cmd_lpush(db, args),
        "llen" => list_cmds::cmd_llen(db, args),
        "lrange" => list_cmds::cmd_lrange(db, args),
        "lpop" => list_cmds::cmd_lpop(db, args),
        "type" => streams_cmds::cmd_type(db, args),
        "xadd" => streams_cmds::cmd_xadd(db, args).await,
        "xrange" => streams_cmds::cmd_xrange(db, args),
        "xread" => streams_cmds::cmd_xread(db, args),
        "info" => repl_cmds::cmd_info(db, args),
        "replconf" => repl_cmds::cmd_replconf(db, args),
        "psync" => repl_cmds::cmd_psync(db, args),
        "config" => persistence_cmds::cmd_config(db, args),
        "zadd" => sets_cmd::cmd_zadd(db, args),
        "zrank" => sets_cmd::cmd_zrank(db, args),
        "zrange" => sets_cmd::cmd_zrange(db, args),
        "zcard" => sets_cmd::cmd_zcard(db, args),
        "zscore" => sets_cmd::cmd_zscore(db, args),
        "zrem" => sets_cmd::cmd_zrem(db, args),
        c => Value::Error(format!("ERR unknown command '{}'", c)),
    }
}
