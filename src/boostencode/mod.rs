use boostencode::parse::parse_val;
use std::cmp;
use std::cmp::Ordering;
use std::collections::HashMap;

#[cfg(test)]
mod test;
mod parse;

#[derive(Debug, PartialEq)]
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
}

impl From<String> for Value {
    fn from(string: String) -> Self {
        Value::decode(string.into_bytes().as_mut())
    }
}

fn compare_bstring(a: &[u8], b: &[u8]) -> Ordering {
    let len = cmp::min(a.len(), b.len());

    for i in 0..len {
        let res = a[i].cmp(&b[i]);
        if res != Ordering::Equal {
            return res;
        }
    }

    a.len().cmp(&b.len())
}