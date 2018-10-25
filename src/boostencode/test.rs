use super::*;

// clone everything because it's a test
#[test]
fn test_encode() {
    let spam = vec![115, 112, 97, 109];
    let hello = vec![104, 101, 108, 108, 111];
    let v1 = Value::BString(spam.clone());
    let v2 = Value::Integer(100);
    let v3 = Value::List(vec![v1.clone(), v2.clone()]);

    let mut map = HashMap::new();
    map.insert(spam, v2.clone());
    map.insert(hello, v3.clone());

    let v4 = Value::Dict(map);

    assert_eq!("4:spam", bstring_to_string(&*v1.encode()));
    assert_eq!("i100e", bstring_to_string(&*v2.encode()));
    assert_eq!("l4:spami100ee", bstring_to_string(&*v3.encode()));
    assert_eq!("d5:hellol4:spami100ee4:spami100ee", bstring_to_string(&*v4.encode()));
}

#[test]
fn test_compare_bstring() {
    let v1 = vec![0, 1, 2, 3];
    let v2 = vec![1, 1, 2, 3];
    let v3 = vec![9, 8, 7];
    let v4 = vec![9, 8, 8];
    let vs = vec![8];
    let vl = vec![8, 8, 8];

    assert_eq!(Ordering::Equal, compare_bytes_slice(v1.as_ref(), v1.as_ref()));
    assert_eq!(Ordering::Less, compare_bytes_slice(v1.as_ref(), v2.as_ref()));
    assert_eq!(Ordering::Greater, compare_bytes_slice(v2.as_ref(), v1.as_ref()));
    assert_eq!(Ordering::Less, compare_bytes_slice(v3.as_ref(), v4.as_ref()));
    assert_eq!(Ordering::Greater, compare_bytes_slice(v4.as_ref(), v3.as_ref()));
    assert_eq!(Ordering::Less, compare_bytes_slice(vs.as_ref(), vl.as_ref()));
    assert_eq!(Ordering::Greater, compare_bytes_slice(vl.as_ref(), vs.as_ref()));
}