use std::io::Cursor;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use super::*;

#[test]
fn test_basic_torrent() {
    let torrent = "d8:announce5:a url4:infod6:lengthi20e4:name9:file name12:piece lengthi100e6:pieces40:\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0fee";
    let info = "d6:lengthi20e4:name9:file name12:piece lengthi100e6:pieces40:\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0fe";
    let mut hash: [u8; 20] = [0; 20];
    let mut hasher = Sha1::new();
    hasher.input(info.as_bytes());
    hasher.result(&mut hash);
    let mut reader = Cursor::new(torrent);
    let meta = MetaInfo::from_torrent(&mut reader).expect("Parsing this torrent should succeed");

    assert_eq!(meta, MetaInfo {
        info_hash: hash,
        info: InfoDict {
            piece_length: 100,
            pieces: vec!["0000000000000000000000000000000000000000".to_owned(), "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f".to_owned()],
            private: false,
            file_info: FileInfo::Single(SingleFile {
                file_name: "file name".to_owned(),
                length: 20,
                md5sum: None
            })
        },
        announce: "a url".to_owned(),
        announce_list: None,
        creation_date: None,
        comment: None,
        created_by: None,
        encoding: None
    });
}