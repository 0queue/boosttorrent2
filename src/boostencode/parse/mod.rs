use boostencode::compare_bytes_slice;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::str::FromStr;
use super::Value;

#[cfg(test)]
mod test;

pub fn parse_val(bytes: &mut Vec<u8>) -> Result<Value, String> {
    match bytes[0] as char {
        'i' => parse_integer(bytes),
        'l' => parse_list(bytes),
        'd' => parse_dict(bytes),
        '0'...'9' => parse_bstring(bytes),
        _ => return Err("Not bencode!".to_string())
    }
}

// assured of a bstring, we take it off the stack of bytes and return it
fn parse_bstring(bytes: &mut Vec<u8>) -> Result<Value, String> {
    let len = parse_integer_literal(bytes)?;
    if bytes.remove(0) as char != ':' {
        return Err("error: expected ':'".to_string());
    }

    let bstring: Vec<u8> = bytes.drain(0..len).collect();

    Ok(Value::BString(bstring))
}

fn parse_integer(bytes: &mut Vec<u8>) -> Result<Value, String> {
    if bytes.remove(0) as char != 'i' {
        return Err("expected i to prefix integer".to_string());
    }

    let is_negative = bytes[0] as char == '-';
    if is_negative {
        bytes.remove(0);
    }

    if bytes[0] as char == '0' {
        if is_negative || bytes[1] as char != 'e' {
            return Err("integer has leading or negative zero".to_string());
        }

        return Ok(Value::Integer(0));
    }

    let num = parse_integer_literal(bytes)? as i32;

    if bytes.remove(0) as char != 'e' {
        return Err("expected e after integer".to_string());
    }

    Ok(Value::Integer(if is_negative { -num } else { num }))
}

fn parse_list(bytes: &mut Vec<u8>) -> Result<Value, String> {
    let mut list = Vec::new();
    if bytes.remove(0) as char != 'l' {
        return Err("expected list prefix".to_string());
    }

    while bytes[0] as char != 'e' {
        list.push(parse_val(bytes)?)
    }

    if bytes.remove(0) as char != 'e' {
        return Err("Expected list postfix".to_string());
    }

    Ok(Value::List(list))
}

fn parse_dict(bytes: &mut Vec<u8>) -> Result<Value, String> {
    let mut map = HashMap::new();

    if bytes.remove(0) as char != 'd' {
        return Err("expected dict prefix".to_string());
    }

    let mut last_key: Option<Vec<u8>> = None;

    while bytes[0] as char != 'e' {
        let key = parse_bstring(bytes)?;
        let val = parse_val(bytes)?;

        if let Value::BString(key) = key {
            if let Some(last) = last_key {
                if compare_bytes_slice(&last, key.as_ref()) != Ordering::Less {
                    return Err("dict keys not in ascending order".to_string());
                }
            }

            last_key = Some(key.clone());
            map.insert(key, val);
        } else {
            return Err("Key is not BString".to_string());
        }
    }

    if bytes.remove(0) as char != 'e' {
        return Err("expected dict postfix".to_string());
    }

    Ok(Value::Dict(map))
}

// parse an integer literal on the top of the stack
fn parse_integer_literal(bytes: &mut Vec<u8>) -> Result<usize, String> {
    let mut num = String::new();
    while bytes.len() > 0 && bytes[0] as char >= '0' && bytes[0] as char <= '9' {
        num.push(bytes.remove(0) as char);
    }

    usize::from_str(num.as_str()).map_err(|_| "error parsing integer literal".to_string())
}