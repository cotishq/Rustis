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
const EARTH_RADIUS_IN_METERS: f64 = 6372797.560856;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

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

fn geohash_get_lat_distance(lat1d: f64, lat2d: f64) -> f64 {
    EARTH_RADIUS_IN_METERS * (lat2d * DEG_TO_RAD - lat1d * DEG_TO_RAD).abs()
}

fn geohash_get_distance(lon1d: f64, lat1d: f64, lon2d: f64, lat2d: f64) -> f64 {
    let lon1r = lon1d * DEG_TO_RAD;
    let lon2r = lon2d * DEG_TO_RAD;
    let v = ((lon2r - lon1r) / 2.0).sin();
    if v == 0.0 {
        return geohash_get_lat_distance(lat1d, lat2d);
    }
    let lat1r = lat1d * DEG_TO_RAD;
    let lat2r = lat2d * DEG_TO_RAD;
    let u = ((lat2r - lat1r) / 2.0).sin();
    let a = u * u + lat1r.cos() * lat2r.cos() * v * v;
    2.0 * EARTH_RADIUS_IN_METERS * a.sqrt().asin()
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

pub fn cmd_geodist(db: &Db, args: &[Value]) -> Value {
    if args.len() < 3 {
        return Value::Error("ERR wrong number of arguments for 'geodist' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    let member1 = match unpack_bulk_str(&args[1]) {
        Ok(m) => m,
        Err(_) => return Value::Error("ERR invalid member".into()),
    };

    let member2 = match unpack_bulk_str(&args[2]) {
        Ok(m) => m,
        Err(_) => return Value::Error("ERR invalid member".into()),
    };

    let score1 = match db.zscore(&key, &member1) {
        Some(s) => s,
        None => return Value::NullBulk
    };

    let score2 = match db.zscore(&key, &member2) {
        Some(s) => s,
        None => return Value::NullBulk,
    };

    let (lat1, lon1) = geohash_decode(score1 as u64);
    let (lat2, lon2) = geohash_decode(score2 as u64);

    let distance = geohash_get_distance(lon1, lat1, lon2, lat2);
    Value::BulkString(format!("{:.4}", distance))
}

fn convert_unit_to_meters(radius: f64, unit: &str) -> Option<f64> {
    match unit.to_lowercase().as_str() {
        "m" => Some(radius),
        "km" => Some(radius * 1000.0),
        "mi" => Some(radius * 1609.34),
        "ft" => Some(radius * 0.3048),
        _ => None,
    }
}

pub fn cmd_geosearch(db: &Db, args: &[Value]) -> Value {
    if args.len() < 6 {
        return Value::Error("ERR wrong number of arguments for 'geosearch' command".into());
    }

    let key = match unpack_bulk_str(&args[0]) {
        Ok(k) => k,
        Err(_) => return Value::Error("ERR invalid key".into()),
    };

    let mut center_lon: Option<f64> = None;
    let mut center_lat: Option<f64> = None;
    let mut radius_meters: Option<f64> = None;

    let mut i = 1;
    while i < args.len() {
        let opt = match unpack_bulk_str(&args[i]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => {
                i += 1;
                continue;
            }
        };

        match opt.as_str() {
            "FROMLONLAT" => {
                if i + 2 >= args.len() {
                    return Value::Error("ERR syntax error".into());
                }
                let lon_str = match unpack_bulk_str(&args[i + 1]) {
                    Ok(s) => s,
                    Err(_) => return Value::Error("ERR invalid longitude".into()),
                };
                let lat_str = match unpack_bulk_str(&args[i + 2]) {
                    Ok(s) => s,
                    Err(_) => return Value::Error("ERR invalid latitude".into()),
                };
                center_lon = lon_str.parse().ok();
                center_lat = lat_str.parse().ok();
                i += 3;
            }
            "BYRADIUS" => {
                if i + 2 >= args.len() {
                    return Value::Error("ERR syntax error".into());
                }
                let radius_str = match unpack_bulk_str(&args[i + 1]) {
                    Ok(s) => s,
                    Err(_) => return Value::Error("ERR invalid radius".into()),
                };
                let unit = match unpack_bulk_str(&args[i + 2]) {
                    Ok(s) => s,
                    Err(_) => return Value::Error("ERR invalid unit".into()),
                };
                let radius: f64 = match radius_str.parse() {
                    Ok(v) => v,
                    Err(_) => return Value::Error("ERR invalid radius".into()),
                };
                radius_meters = convert_unit_to_meters(radius, &unit);
                if radius_meters.is_none() {
                    return Value::Error("ERR unsupported unit".into());
                }
                i += 3;
            }
            _ => {
                i += 1;
            }
        }
    }

    let center_lon = match center_lon {
        Some(v) => v,
        None => return Value::Error("ERR FROMLONLAT is required".into()),
    };
    let center_lat = match center_lat {
        Some(v) => v,
        None => return Value::Error("ERR FROMLONLAT is required".into()),
    };
    let radius_meters = match radius_meters {
        Some(v) => v,
        None => return Value::Error("ERR BYRADIUS is required".into()),
    };

    let members = db.zrange(&key, 0, -1);
    let mut results = Vec::new();

    for member in members {
        if let Some(score) = db.zscore(&key, &member) {
            let (lat, lon) = geohash_decode(score as u64);
            let distance = geohash_get_distance(center_lon, center_lat, lon, lat);
            if distance <= radius_meters {
                results.push(Value::BulkString(member));
            }
        }
    }

    Value::Array(results)
}

