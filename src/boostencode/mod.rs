use std::cmp;
use std::cmp::Ordering;
use std::collections::HashMap;

#[cfg(test)]
mod test;

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

fn parse_val(bytes: &mut Vec<u8>) -> Value {
    match bytes[0] as char {
        'i' => parse_integer(bytes),
        'l' => parse_list(bytes),
        'd' => parse_dict(bytes),
        '0'...'9' => parse_bstring(bytes),
        _ => panic!("Not bencode!")
    }
}

// assured of a bstring, we take it off the stack of bytes and return it
fn parse_bstring(bytes: &mut Vec<u8>) -> Value {
    let len = parse_integer_literal(bytes);
    if bytes.remove(0) as char != ':' {
        panic!("error: expected ':'");
    }

    let bstring: Vec<u8> = bytes.drain(0..len).collect();

    Value::BString(bstring)
}

fn parse_integer(bytes: &mut Vec<u8>) -> Value {
    if bytes.remove(0) as char != 'i' {
        panic!("expected i to prefix integer");
    }

    let is_negative = bytes[0] as char == '-';
    if is_negative {
        bytes.remove(0);
    }

    if bytes[0] as char == '0' {
        if is_negative || bytes[1] as char != 'e' {
            panic!("integer has leading or negative zero");
        }

        return Value::Integer(0);
    }

    let num = parse_integer_literal(bytes) as i32;

    if bytes.remove(0) as char != 'e' {
        panic!("expected e after integer");
    }

    Value::Integer(if is_negative { -num } else { num })
}

fn parse_list(bytes: &mut Vec<u8>) -> Value {
    let mut list = Vec::new();
    if bytes.remove(0) as char != 'l' {
        panic!("expected list prefix");
    }

    while bytes[0] as char != 'e' {
        list.push(parse_val(bytes))
    }

    if bytes.remove(0) as char != 'e' {
        panic!("Expected list postfix");
    }

    Value::List(list)
}

fn parse_dict(bytes: &mut Vec<u8>) -> Value {
    let mut map = HashMap::new();

    if bytes.remove(0) as char != 'd' {
        panic!("expected dict prefix");
    }

    let mut last_key: Option<Vec<u8>> = None;

    while bytes[0] as char != 'e' {
        let key = parse_bstring(bytes);
        let val = parse_val(bytes);

        if let Value::BString(key) = key {
            if let Some(last) = last_key {
                if compare_bstring(&last, key.as_ref()) != Ordering::Less {
                    panic!("dict keys not in ascending order");
                }
            }

            last_key = Some(key.clone());
            map.insert(key, val);
        } else {
            panic!("Key is not BString");
        }
    }

    if bytes.remove(0) as char != 'e' {
        panic!("expected dict postfix");
    }

    Value::Dict(map)
}

// parse an integer literal on the top of the stack
fn parse_integer_literal(bytes: &mut Vec<u8>) -> usize {
    let mut num = String::new();
    while bytes.len() > 0 && bytes[0] as char >= '0' && bytes[0] as char <= '9' {
        num.push(bytes.remove(0) as char);
    }

    // TODO better errors

    num.parse().unwrap()
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