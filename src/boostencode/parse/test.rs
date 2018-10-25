use super::*;

#[test]
fn test_parse_integer_literal() {
    let mut s123 = "123e".to_string().into_bytes();
    let res = parse_integer_literal(s123.as_mut()).unwrap();
    assert_eq!(res, 123);
}

#[test]
fn test_parse_bstring() {
    let mut s1 = "4:spam".to_string().into_bytes();

    let val = parse_bstring(s1.as_mut()).unwrap();

    assert_eq!(val, Value::BString(vec!['s' as u8, 'p' as u8, 'a' as u8, 'm' as u8]));
    assert_eq!(0, s1.len());
}

#[test]
fn test_parse_integer() {
    let mut s1 = "i123e".to_string().into_bytes();
    let mut s2 = "i-4e".to_string().into_bytes();

    let val1 = parse_integer(s1.as_mut()).unwrap();
    let val2 = parse_integer(s2.as_mut()).unwrap();

    assert_eq!(val1, Value::Integer(123));
    assert_eq!(val2, Value::Integer(-4));
    assert_eq!(0, s1.len());
    assert_eq!(0, s2.len());
}

#[test]
#[should_panic]
fn test_parse_integer_negative_zero() {
    let mut s1 = "i-0e".to_string().into_bytes();
    parse_integer(s1.as_mut()).unwrap();
}

#[test]
#[should_panic]
fn test_parse_integer_leading_zero() {
    let mut s1 = "i023e".to_string().into_bytes();
    parse_integer(s1.as_mut()).unwrap();
}

#[test]
fn test_parse_list() {
    let mut s1 = "l4:spami123ee".to_string().into_bytes();

    let val1 = parse_list(s1.as_mut()).unwrap();

    assert_eq!(val1, Value::List(vec![Value::BString(vec!['s' as u8, 'p' as u8, 'a' as u8, 'm' as u8]), Value::Integer(123)]))
}

#[test]
fn test_parses_dict() {
    let mut s1 = "d5:hello5:world4:spami123ee".to_string().into_bytes();
    let val1 = parse_dict(s1.as_mut()).unwrap();

    let mut map = HashMap::new();
    map.insert(vec!['h' as u8, 'e' as u8, 'l' as u8, 'l' as u8, 'o' as u8], Value::BString(vec!['w' as u8, 'o' as u8, 'r' as u8, 'l' as u8, 'd' as u8]));
    map.insert(vec!['s' as u8, 'p' as u8, 'a' as u8, 'm' as u8], Value::Integer(123));
    assert_eq!(val1, Value::Dict(map));
}

#[test]
#[should_panic]
fn test_parse_dict_not_ascending() {
    let mut s1 = "d5:worldi1e5:helloi2ee".to_string().into_bytes();
    parse_dict(s1.as_mut()).unwrap();
}