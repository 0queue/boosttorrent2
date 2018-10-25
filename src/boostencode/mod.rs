use boostencode::parse::parse_val;
use std::cmp;
use std::cmp::Ordering;
use std::collections::HashMap;

#[cfg(test)]
mod test;
mod parse;

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    BString(Vec<u8>),
    Integer(i32),
    List(Vec<Value>),
    Dict(HashMap<Vec<u8>, Value>),
}

impl Value {
    pub fn decode(bytes: &[u8]) -> Value {
        let mut bytes: Vec<u8> = Vec::from(bytes);
        let val = parse_val(&mut bytes);

        if bytes.len() > 0 {
            panic!("extra bytes");
        }

        val
    }

    pub fn encode(&self) -> Vec<u8> {
        match self {
            Value::BString(bytes) => {
                let mut res: Vec<_> = string_to_byte_vec(bytes.len().to_string());
                res.push(':' as u8);
                // clone because we are borrowing self, want to give it back after
                res.append(&mut bytes.clone());

                res
            }
            Value::Integer(num) => string_to_byte_vec(format!("i{}e", num)),
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

impl From<String> for Value {
    fn from(string: String) -> Self {
        Value::decode(string.into_bytes().as_mut())
    }
}

fn string_to_byte_vec(string: String) -> Vec<u8> {
    string.chars().map(|c| c as u8).collect()
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