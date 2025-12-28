use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct RdbEntry {
    pub value: String,
    pub expire_at: Option<SystemTime>,
}

pub struct RdbParser {
    reader: BufReader<File>,
}

impl RdbParser {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self {
            reader: BufReader::new(file),
        })
    }

    pub fn parse(&mut self) -> std::io::Result<HashMap<String, RdbEntry>> {
        let mut entries = HashMap::new();

        // Read and verify header (REDIS0011)
        let mut header = [0u8; 9];
        self.reader.read_exact(&mut header)?;
        
        let header_str = String::from_utf8_lossy(&header);
        if !header_str.starts_with("REDIS") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid RDB file header",
            ));
        }

        // Parse sections
        loop {
            let mut op = [0u8; 1];
            if self.reader.read_exact(&mut op).is_err() {
                break;
            }

            match op[0] {
                0xFA => {
                    // Metadata subsection - skip it
                    let _name = self.read_string()?;
                    let _value = self.read_string()?;
                }
                0xFE => {
                    // Database selector
                    let _db_index = self.read_length()?;
                }
                0xFB => {
                    // Hash table size info
                    let _hash_table_size = self.read_length()?;
                    let _expire_hash_table_size = self.read_length()?;
                }
                0xFC => {
                    // Expire time in milliseconds
                    let mut ms_bytes = [0u8; 8];
                    self.reader.read_exact(&mut ms_bytes)?;
                    let expire_ms = u64::from_le_bytes(ms_bytes);
                    let expire_at = UNIX_EPOCH + Duration::from_millis(expire_ms);

                    // Read value type
                    let mut value_type = [0u8; 1];
                    self.reader.read_exact(&mut value_type)?;

                    if value_type[0] == 0 {
                        // String type
                        let key = self.read_string()?;
                        let value = self.read_string()?;
                        entries.insert(key, RdbEntry {
                            value,
                            expire_at: Some(expire_at),
                        });
                    }
                }
                0xFD => {
                    // Expire time in seconds
                    let mut sec_bytes = [0u8; 4];
                    self.reader.read_exact(&mut sec_bytes)?;
                    let expire_sec = u32::from_le_bytes(sec_bytes);
                    let expire_at = UNIX_EPOCH + Duration::from_secs(expire_sec as u64);

                    // Read value type
                    let mut value_type = [0u8; 1];
                    self.reader.read_exact(&mut value_type)?;

                    if value_type[0] == 0 {
                        // String type
                        let key = self.read_string()?;
                        let value = self.read_string()?;
                        entries.insert(key, RdbEntry {
                            value,
                            expire_at: Some(expire_at),
                        });
                    }
                }
                0xFF => {
                    // End of file
                    break;
                }
                0x00 => {
                    // String value type (no expiry)
                    let key = self.read_string()?;
                    let value = self.read_string()?;
                    entries.insert(key, RdbEntry {
                        value,
                        expire_at: None,
                    });
                }
                _ => {
                    // Unknown opcode, try to continue
                }
            }
        }

        Ok(entries)
    }

    fn read_length(&mut self) -> std::io::Result<usize> {
        let mut first_byte = [0u8; 1];
        self.reader.read_exact(&mut first_byte)?;

        let first = first_byte[0];
        let encoding_type = (first & 0xC0) >> 6;

        match encoding_type {
            0 => {
                // 6-bit length
                Ok((first & 0x3F) as usize)
            }
            1 => {
                // 14-bit length
                let mut second_byte = [0u8; 1];
                self.reader.read_exact(&mut second_byte)?;
                let length = ((first & 0x3F) as usize) << 8 | second_byte[0] as usize;
                Ok(length)
            }
            2 => {
                // 32-bit length
                let mut len_bytes = [0u8; 4];
                self.reader.read_exact(&mut len_bytes)?;
                Ok(u32::from_be_bytes(len_bytes) as usize)
            }
            3 => {
                // Special encoding - return the remaining 6 bits as a marker
                Ok((first & 0x3F) as usize | 0x8000_0000)
            }
            _ => unreachable!(),
        }
    }

    fn read_string(&mut self) -> std::io::Result<String> {
        let length = self.read_length()?;

        // Check for special encoding (0b11 prefix)
        if length & 0x8000_0000 != 0 {
            let encoding = length & 0x3F;
            match encoding {
                0 => {
                    // 8-bit integer
                    let mut byte = [0u8; 1];
                    self.reader.read_exact(&mut byte)?;
                    Ok((byte[0] as i8).to_string())
                }
                1 => {
                    // 16-bit integer (little-endian)
                    let mut bytes = [0u8; 2];
                    self.reader.read_exact(&mut bytes)?;
                    Ok((i16::from_le_bytes(bytes)).to_string())
                }
                2 => {
                    // 32-bit integer (little-endian)
                    let mut bytes = [0u8; 4];
                    self.reader.read_exact(&mut bytes)?;
                    Ok((i32::from_le_bytes(bytes)).to_string())
                }
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Unsupported string encoding: {}", encoding),
                )),
            }
        } else {
            // Regular string
            let mut buffer = vec![0u8; length];
            self.reader.read_exact(&mut buffer)?;
            String::from_utf8(buffer).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })
        }
    }
}

pub fn load_rdb(dir: Option<&str>, dbfilename: Option<&str>) -> HashMap<String, RdbEntry> {
    let dir = match dir {
        Some(d) => d,
        None => return HashMap::new(),
    };

    let filename = match dbfilename {
        Some(f) => f,
        None => return HashMap::new(),
    };

    let path = Path::new(dir).join(filename);
    
    if !path.exists() {
        return HashMap::new();
    }

    match RdbParser::open(&path) {
        Ok(mut parser) => match parser.parse() {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("Failed to parse RDB file: {}", e);
                HashMap::new()
            }
        },
        Err(e) => {
            eprintln!("Failed to open RDB file: {}", e);
            HashMap::new()
        }
    }
}
