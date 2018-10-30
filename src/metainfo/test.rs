use super::*;

#[test]
fn test_trivial() {
    let val = Value::Integer(0);
    let meta = MetaInfo::from_value(val);
    assert_eq!(None, meta);
}