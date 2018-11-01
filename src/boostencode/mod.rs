use boostencode::parse::parse_val;
use derive_error::Error;
use serde;
use std::cmp;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Display;
use std::fmt::Error;
use std::fmt::Formatter;
use std::str;

#[cfg(test)]
mod test;
mod parse;
mod ser;
mod de;

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    BString(Vec<u8>),
    Integer(i32),
    List(Vec<Value>),
    Dict(HashMap<Vec<u8>, Value>),
}

pub type Result<T> = std::result::Result<T, DecodeError>;

// obviously this will not do right now but its a start
#[derive(Debug, Error, PartialEq)]
pub enum DecodeError {
    /// The encoded string was not formatted correctly
    InvalidValue,
    /// Error parsing string value
    InvalidString,
    /// Error parsing integer value
    InvalidInteger,
    /// Error parsing list value
    InvalidList,
    /// Error parsing dict value
    InvalidDict,
}

impl serde::ser::Error for DecodeError {
    fn custom<T: Display>(msg: T) -> Self {
        DecodeError::InvalidString
    }
}

impl serde::de::Error for DecodeError {
    fn custom<T: Display>(msg: T) -> Self {
        DecodeError::InvalidString
    }
}


impl Value {
    pub fn decode(bytes: &[u8]) -> Result<Value> {
        let mut bytes: Vec<u8> = Vec::from(bytes);
        let val = parse_val(&mut bytes)?;

        if bytes.len() > 0 {
            return Err(DecodeError::InvalidValue);
        }

        Ok(val)
    }

    pub fn encode(&self) -> Vec<u8> {
        match self {
            Value::BString(bytes) => {
                let mut res = Vec::from(bytes.len().to_string().as_bytes());
                res.push(':' as u8);
                // clone because we are borrowing self, want to give it back after
                res.append(&mut bytes.clone());

                res
            }
            Value::Integer(num) => Vec::from(format!("i{}e", num).as_bytes()),
            Value::List(vals) => {
                let mut res = vec!['l' as u8];
                vals.into_iter().for_each(|v| res.append(&mut v.encode()));
                res.push('e' as u8);

                res
            }
            Value::Dict(map) => {
                let mut res = vec!['d' as u8];

                let mut keys: Vec<_> = map.keys().collect();
                keys.sort_by(|a, b| compare_bytes_slice(a, b));
                keys.into_iter().for_each(|key| {
                    res.append(&mut Value::BString(key.clone()).encode());
                    res.append(&mut map.get(key).unwrap().encode());
                });

                res.push('e' as u8);

                res
            }
        }
    }
}

impl Display for Value {
    // TODO proper indentation
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), Error> {
        match self {
            Value::BString(bytes) => write!(f, "{}", str::from_utf8(bytes).unwrap_or(format!("<{} bytes>", bytes.len()).as_ref())),
            Value::Integer(num) => write!(f, "{}", num),
            Value::List(vals) => {
                write!(f, "[")?;
                vals.iter().enumerate().for_each(|(i, val)| {
                    if i > 0 {
                        write!(f, ", ");
                    }

                    write!(f, "{}", val);
                });
                write!(f, "]")
            }
            Value::Dict(map) => {
                let mut entries: Vec<_> = map.iter().collect();
                entries.sort_by(|(k1, _), (k2, _)| compare_bytes_slice(*k1, *k2));
                writeln!(f, "{{");
                entries.iter().for_each(|(k, v)| {
                    writeln!(f, "  {} => {}", str::from_utf8(k).unwrap_or("[...bytes...]"), v);
                });
                write!(f, "}}")
            }
        }
    }
}

fn compare_bytes_slice(a: &[u8], b: &[u8]) -> Ordering {
    let len = cmp::min(a.len(), b.len());

    for i in 0..len {
        let res = a[i].cmp(&b[i]);
        if res != Ordering::Equal {
            return res;
        }
    }

    a.len().cmp(&b.len())
}