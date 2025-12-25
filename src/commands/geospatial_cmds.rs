use crate::resp::Value;
use crate::db::Db;

fn unpack_bulk_str(value: &Value) -> Result<String, anyhow::Error> {
    match value {
        Value::BulkString(s) => Ok(s.clone()),
        _ => Err(anyhow::anyhow!("Expected bulk string")),
    }
}

const MIN_LONGITUDE: f64 = -180.0;
const MAX_LONGITUDE: f64 = 180.0;
const MIN_LATITUDE: f64 = -85.05112878;
const MAX_LATITUDE: f64 = 85.05112878;

pub fn cmd_geoadd(db: &Db, args: &[Value]) -> Value {
    if args.len() < 4 {
        return Value::Error("ERR wrong number of arguments for 'geoadd' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    let longitude_str = match unpack_bulk_str(&args[1]) {
        Ok(s) => s,
        Err(_) => return Value::Error("ERR invalid longitude".into()), 
    };

    let longitude: f64 = match longitude_str.parse() {
        Ok(v) => v,
        Err(_) => return Value::Error("ERR value is not a valid float".into()),
    };

    let latitude_str = match unpack_bulk_str(&args[2]) {
        Ok(s) => s,
        Err(_) => return Value::Error("ERR invalid latitude".into()), 
    };

    let latitude: f64 = match latitude_str.parse() {
        Ok(v) => v,
        Err(_) => return Value::Error("ERR value is not a valid float".into()),
    };

    if longitude < MIN_LONGITUDE || longitude > MAX_LONGITUDE ||
       latitude < MIN_LATITUDE || latitude > MAX_LATITUDE {
        return Value::Error(format!(
            "ERR invalid longitude,latitude pair {:.6},{:.6}",
            longitude, latitude
        ));
    }
    let member = match unpack_bulk_str(&args[3]) {
        Ok(m) => m,
        Err(_) => return Value::Error("ERR invalid member".into()),
    };
    let score = 0.0;
    let added = db.zadd(key, score, member);
    Value::Integer(added as i64)
}

