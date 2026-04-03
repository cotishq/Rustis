# Rustis

A Redis clone built from scratch in Rust. This project implements core Redis functionality including data structures, persistence, replication, pub/sub messaging, and more.

## 🚀 Demo

[![Watch the demo](https://img.youtube.com/vi/NbdJff_rRMY/0.jpg)](https://youtu.be/NbdJff_rRMY)

## Features

### Data Structures

#### Strings
- `SET key value [PX milliseconds]` - Set a key with optional expiration
- `GET key` - Get the value of a key
- `INCR key` - Increment the integer value of a key

#### Lists
- `LPUSH key value [value ...]` - Prepend values to a list
- `RPUSH key value [value ...]` - Append values to a list
- `LPOP key [count]` - Remove and return elements from the head
- `LRANGE key start stop` - Get a range of elements
- `LLEN key` - Get the length of a list

#### Sorted Sets
- `ZADD key score member` - Add a member with score to a sorted set
- `ZRANGE key start stop` - Get members by index range
- `ZRANK key member` - Get the rank of a member
- `ZSCORE key member` - Get the score of a member
- `ZCARD key` - Get the number of members
- `ZREM key member` - Remove a member

#### Streams
- `XADD key id field value [field value ...]` - Append an entry to a stream
- `XRANGE key start end` - Get a range of entries
- `XREAD STREAMS key id` - Read entries from streams

### Pub/Sub Messaging
- `SUBSCRIBE channel [channel ...]` - Subscribe to channels
- `UNSUBSCRIBE channel [channel ...]` - Unsubscribe from channels
- `PUBLISH channel message` - Publish a message to a channel

### Geospatial
- `GEOADD key longitude latitude member` - Add geospatial items
- `GEOPOS key member [member ...]` - Get positions of members
- `GEODIST key member1 member2 [unit]` - Get distance between members
- `GEOSEARCH key FROMMEMBER member BYRADIUS radius unit` - Search for members within radius

### Transactions
- `MULTI` - Start a transaction
- `EXEC` - Execute queued commands
- `DISCARD` - Discard queued commands

### Persistence
- **RDB Loading** - Automatically loads data from RDB files on startup
- `CONFIG GET dir` - Get the database directory
- `CONFIG GET dbfilename` - Get the RDB filename
- `KEYS *` - Get all keys in the database

### Replication
- Master/Slave replication support
- `INFO replication` - Get replication information
- `REPLCONF` - Replication configuration
- `PSYNC` - Partial synchronization

### Authentication
- `AUTH password` - Authenticate to the server
- `ACL` - Access control list management

### General Commands
- `PING` - Test connection
- `ECHO message` - Echo a message
- `TYPE key` - Get the type of a key

## Getting Started

### Prerequisites
- Rust (latest stable version)
- Cargo

### Building

```bash
cargo build --release
```

### Running

Basic server:
```bash
cargo run
```

With custom port:
```bash
cargo run -- --port 6380
```

With RDB persistence:
```bash
cargo run -- --dir /path/to/data --dbfilename dump.rdb
```

As a replica:
```bash
cargo run -- --port 6380 --replicaof "127.0.0.1 6379"
```

### Connecting

Use `redis-cli` or any Redis client:
```bash
redis-cli -p 6379
```


## Architecture

```
src/
├── main.rs              # Server entry point and connection handling
├── resp/                # RESP protocol implementation
│   ├── parser.rs        # RESP message parser
│   ├── serializer.rs    # RESP message serializer
│   └── types.rs         # RESP data types
├── db/                  # Database core
│   └── core.rs          # In-memory data store with expiration
├── commands/            # Command implementations
│   ├── dispatcher.rs    # Command routing
│   ├── string_cmds.rs   # String commands
│   ├── list_cmds.rs     # List commands
│   ├── sets_cmd.rs      # Sorted set commands
│   ├── streams_cmds.rs  # Stream commands
│   ├── pubsub_cmds.rs   # Pub/Sub commands
│   ├── geospatial_cmds.rs # Geospatial commands
│   ├── persistence_cmds.rs # Persistence commands
│   ├── repl_cmds.rs     # Replication commands
│   └── auth_cmds.rs     # Authentication commands
├── rdb/                 # RDB persistence
│   └── parser.rs        # RDB file parser
├── replication/         # Replication support
│   └── handshake.rs     # Master-slave handshake
└── client.rs            # Client state management
```

## Technical Highlights

- **Async I/O** - Built with Tokio for high-performance async networking
- **RESP Protocol** - Full implementation of Redis Serialization Protocol
- **Memory Efficient** - Uses `bytes::Bytes` for zero-copy operations
- **Concurrent Access** - Thread-safe data structures with `Arc<Mutex<>>`
- **Background Tasks** - Automatic key expiration via background task
- **RDB Compatibility** - Reads standard Redis RDB files (version 11)

## Command Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `--port <port>` | Server port | 6379 |
| `--dir <path>` | Directory for RDB files | None |
| `--dbfilename <name>` | RDB filename | None |
| `--replicaof "<host> <port>"` | Run as replica of master | None |


## Acknowledgments

- Inspired by [Redis](https://redis.io/)
