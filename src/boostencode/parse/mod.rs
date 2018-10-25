use boostencode::compare_bytes_slice;
use std::cmp::Ordering;
use std::collections::HashMap;
use super::Value;

#[cfg(test)]
mod test;

pub fn parse_val(bytes: &mut Vec<u8>) -> Value {
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
                if compare_bytes_slice(&last, key.as_ref()) != Ordering::Less {
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