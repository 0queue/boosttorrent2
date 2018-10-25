use super::*;

#[test]
fn test_compare_bstring() {
    let v1 = vec![0, 1, 2, 3];
    let v2 = vec![1, 1, 2, 3];
    let v3 = vec![9, 8, 7];
    let v4 = vec![9, 8, 8];
    let vs = vec![8];
    let vl = vec![8, 8, 8];

    assert_eq!(Ordering::Equal, compare_bstring(v1.as_ref(), v1.as_ref()));
    assert_eq!(Ordering::Less, compare_bstring(v1.as_ref(), v2.as_ref()));
    assert_eq!(Ordering::Greater, compare_bstring(v2.as_ref(), v1.as_ref()));
    assert_eq!(Ordering::Less, compare_bstring(v3.as_ref(), v4.as_ref()));
    assert_eq!(Ordering::Greater, compare_bstring(v4.as_ref(), v3.as_ref()));
    assert_eq!(Ordering::Less, compare_bstring(vs.as_ref(), vl.as_ref()));
    assert_eq!(Ordering::Greater, compare_bstring(vl.as_ref(), vs.as_ref()));
}