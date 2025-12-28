use std::collections::HashSet;
use tokio::sync::mpsc;

use crate::db::{Db, PubSubMessage};
use crate::resp::Value;

pub fn cmd_subscribe(
    args: &[Value],
    subscribed_channels: &mut HashSet<String>,
    pubsub_tx: &mpsc::Sender<PubSubMessage>,
    db: &Db,
) -> Value {
    if args.is_empty() {
        return Value::Error("ERR wrong number of arguments for 'subscribe' command".into());
    }

    let channel = match &args[0] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::Error("ERR invalid channel name".into()),
    };

    // Only subscribe if not already subscribed to this channel
    if !subscribed_channels.contains(&channel) {
        db.subscribe_channel(&channel, pubsub_tx.clone());
        subscribed_channels.insert(channel.clone());
    }

    Value::Array(vec![
        Value::BulkString("subscribe".into()),
        Value::BulkString(channel),
        Value::Integer(subscribed_channels.len() as i64),
    ])
}

pub fn cmd_publish(args: &[Value], db: &Db) -> Value {
    if args.len() < 2 {
        return Value::Error("ERR wrong number of arguments for 'publish' command".into());
    }

    let channel = match &args[0] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::Error("ERR invalid channel name".into()),
    };

    let message = match &args[1] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::Error("ERR invalid message".into()),
    };

    // Publish and return the number of subscribers that received the message
    let count = db.publish_message(&channel, &message);
    Value::Integer(count as i64)
}

pub fn cmd_unsubscribe(
    args: &[Value],
    subscribed_channels: &mut HashSet<String>,
) -> Value {
    if args.is_empty() {
        return Value::Error("ERR wrong number of arguments for 'unsubscribe' command".into());
    }

    let channel = match &args[0] {
        Value::BulkString(s) => s.clone(),
        _ => return Value::Error("ERR invalid channel name".into()),
    };

    // Remove from local tracking (actual channel cleanup happens when sender is dropped/closed)
    subscribed_channels.remove(&channel);

    Value::Array(vec![
        Value::BulkString("unsubscribe".into()),
        Value::BulkString(channel),
        Value::Integer(subscribed_channels.len() as i64),
    ])
}
