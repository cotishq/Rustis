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
const LATITUDE_RANGE: f64 = MAX_LATITUDE - MIN_LATITUDE;
const LONGITUDE_RANGE: f64 = MAX_LONGITUDE - MIN_LONGITUDE;

fn spread_int32_to_int64(v: u32) -> u64 {
    let mut result = v as u64;
    result = (result | (result << 16)) & 0x0000FFFF0000FFFF;
    result = (result | (result << 8)) & 0x00FF00FF00FF00FF;
    result = (result | (result << 4)) & 0x0F0F0F0F0F0F0F0F;
    result = (result | (result << 2)) & 0x3333333333333333;
    (result | (result << 1)) & 0x5555555555555555
}

fn interleave(x: u32, y: u32) -> u64 {
    let x_spread = spread_int32_to_int64(x);
    let y_spread = spread_int32_to_int64(y);
    let y_shifted = y_spread << 1;
    x_spread | y_shifted
}

fn geohash_encode(latitude: f64, longitude: f64) -> u64 {
    let normalized_latitude = 2.0_f64.powi(26) * (latitude - MIN_LATITUDE) / LATITUDE_RANGE;
    let normalized_longitude = 2.0_f64.powi(26) * (longitude - MIN_LONGITUDE) / LONGITUDE_RANGE;
    let lat_int = normalized_latitude as u32;
    let lon_int = normalized_longitude as u32;
    interleave(lat_int, lon_int)
}

fn compact_int64_to_int32(v: u64) -> u32 {
    let mut result = v & 0x5555555555555555;
    result = (result | (result >> 1)) & 0x3333333333333333;
    result = (result | (result >> 2)) & 0x0F0F0F0F0F0F0F0F;
    result = (result | (result >> 4)) & 0x00FF00FF00FF00FF;
    result = (result | (result >> 8)) & 0x0000FFFF0000FFFF;
    ((result | (result >> 16)) & 0x00000000FFFFFFFF) as u32
}

fn geohash_decode(geo_code: u64) -> (f64, f64) {
    let y = geo_code >> 1;
    let x = geo_code;

    let grid_lat = compact_int64_to_int32(x);
    let grid_lon = compact_int64_to_int32(y);

    let grid_lat_min = MIN_LATITUDE + LATITUDE_RANGE * (grid_lat as f64 / 2.0_f64.powi(26));
    let grid_lat_max = MIN_LATITUDE + LATITUDE_RANGE * ((grid_lat + 1) as f64 / 2.0_f64.powi(26));
    let grid_lon_min = MIN_LONGITUDE + LONGITUDE_RANGE * (grid_lon as f64 / 2.0_f64.powi(26));
    let grid_lon_max = MIN_LONGITUDE + LONGITUDE_RANGE * ((grid_lon + 1) as f64 / 2.0_f64.powi(26));

    let latitude = (grid_lat_min + grid_lat_max) / 2.0;
    let longitude = (grid_lon_min + grid_lon_max) / 2.0;

    (latitude, longitude)
}

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
    
    let score = geohash_encode(latitude, longitude) as f64;
    let added = db.zadd(key, score, member);
    Value::Integer(added as i64)
}

pub fn cmd_geopos(db: &Db, args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Error("ERR wrong number of arguments for 'geopos' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    let mut results = Vec::new();

    for arg in &args[1..] {
        let member = match unpack_bulk_str(arg) {
            Ok(m) => m,
            Err(_) => {
                results.push(Value::NullArray);
                continue;
            }
        };

        match db.zscore(&key, &member) {
            Some(score) => {
                let (latitude, longitude) = geohash_decode(score as u64);
                results.push(Value::Array(vec![
                    Value::BulkString(longitude.to_string()),
                    Value::BulkString(latitude.to_string()),
                ]));
            }
            None => {
                results.push(Value::NullArray);
            }
        }
    }

    Value::Array(results)
}

