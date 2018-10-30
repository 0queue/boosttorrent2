use maplit::hashmap;
use super::*;

fn bytes(s: &str) -> Vec<u8> {
    Vec::from(s.as_bytes())
}

#[test]
fn test_metainfo_from_value_trivial_invalid() {
    let val = Value::Integer(0);
    let meta = MetaInfo::from_value(val);
    assert_eq!(None, meta);
}

#[test]
fn test_metainfo_from_value_valid() {
    let info = Value::Dict(hashmap! {
        bytes("piece length") => Value::Integer(20),
        bytes("pieces") => Value::BString(vec![0, 1, 2, 3]),
        bytes("length") => Value::Integer(100),
        bytes("name") => Value::BString(Vec::from("test_file.mp3".as_bytes())),
    });

    let val = Value::Dict(hashmap! {
        bytes("announce") => Value::BString(bytes("http://example.com")),
        bytes("info") => info.clone()
    });

    assert_eq!(MetaInfo::from_value(val), Some(MetaInfo {
        info_hash: sha1_hash(info.encode().as_ref()),
        info: InfoDict {
            piece_length: 20,
            pieces: vec!["00010203".to_string()],
            private: false,
            file_info: FileInfo::Single(SingleFile {
                file_name: "test_file.mp3".to_string(),
                length: 100,
                md5sum: None,
            }),
        },
        announce: "http://example.com".to_string(),
        announce_list: None,
        creation_date: None,
        comment: None,
        created_by: None,
        encoding: None,
    }));
}