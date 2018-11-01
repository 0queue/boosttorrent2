use serde_derive::Serialize;
use serde_bytes::Bytes;
use std::collections::HashMap;
use super::*;

#[test]
fn test_integers() {
    let zero = to_string(&0).unwrap();
    let one = to_string(&1).unwrap();
    let big = to_string(&123456).unwrap();
    let negative_one = to_string(&-1).unwrap();
    let small = to_string(&-654321).unwrap();

    assert_eq!("i0e".to_string(), zero);
    assert_eq!("i1e".to_string(), one);
    assert_eq!("i123456e".to_string(), big);
    assert_eq!("i-1e".to_string(), negative_one);
    assert_eq!("i-654321e".to_string(), small);
}

#[test]
fn test_bstrings() {
    let hello = to_string(&"hello".to_string()).unwrap();
    let nothing = to_string(&"".to_string()).unwrap();
    // known issue relating to specialization, use the serde_bytes crate to force bstring encoding
    // of byte arrays and Vec<u8>
    let bytes = to_bytes(&Bytes::new(&[1, 2, 3])).unwrap();
    let bytes_expected = vec!['3' as u8, ':' as u8, 1, 2, 3];

    let long = to_string(&"loooooooooong".to_string()).unwrap();

    assert_eq!("5:hello".to_string(), hello);
    assert_eq!("0:".to_string(), nothing);
    assert_eq!(bytes_expected, bytes);
    assert_eq!("13:loooooooooong".to_string(), long);
}

#[test]
fn test_list() {
    let numbers = to_string(&vec![1, 2, 3]).unwrap();
    let letters = to_string(&vec!["hello".to_string(), "world".to_string()]).unwrap();
    let list_of_lists = to_string(&vec![vec!["list1".to_string()], vec!["list2".to_string()]]).unwrap();

    assert_eq!("li1ei2ei3ee".to_string(), numbers);
    assert_eq!("l5:hello5:worlde".to_string(), letters);
    assert_eq!("ll5:list1el5:list2ee".to_string(), list_of_lists);
}

#[test]
fn test_map() {
    let mut map = HashMap::new();
    map.insert("hello", 5);
    let serialized = to_string(&map).unwrap();

    assert_eq!("d5:helloi5ee".to_string(), serialized);
}

#[test]
fn test_struct() {
    #[derive(Serialize)]
    struct Test {
        zinteger: i32,
        string: String,
        list: Vec<String>,
    }

    let test = Test {
        zinteger: 555,
        string: "struct time".to_string(),
        list: vec!["one".to_string(), "two".to_string(), "three".to_string()],
    };

    let res = to_string(&test).unwrap();
    let serialized = "d4:listl3:one3:two5:threee6:string11:struct time8:zintegeri555ee".to_string();
}