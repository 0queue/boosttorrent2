//! metainfo contains functions and types to parse the .torrent file
use boostencode::{FromValue, Value};
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use std::collections::HashMap;

#[cfg(test)]
mod test;

#[derive(Debug, PartialEq, Clone)]
pub struct SingleFile {
    // Full path of the file from the root
    pub file_name: String,
    // File size
    pub length: usize,
    // MD5 Sum of the entire file
    pub md5sum: Option<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct MultiFile {
    // Name of the root directory of the torrent
    pub root_dir_name: String,
    // A list of all files in this torrent
    pub files: Vec<SingleFile>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum FileInfo {
    Single(SingleFile),
    Multi(MultiFile),
}

#[derive(Debug, PartialEq, Clone)]
pub struct InfoDict {
    // The number of bytes in each piece
    pub piece_length: usize,
    // The SHA1 hashes of each piece
    pub pieces: Vec<String>,
    // If true, only publish presence via trackers and not directly to peers
    pub private: bool,
    // Information about the file(s) to download
    pub file_info: FileInfo,
}

#[derive(Debug, PartialEq, Clone)]
pub struct MetaInfo {
    // The SHA1 hash of the value of the info key in the torrent file
    pub info_hash: [u8; 20],
    // Information about the file to be downloaded
    pub info: InfoDict,
    // The url for the tracker
    pub announce: String,
    // An optional list of more trackers, the lower the usize the higher priority
    pub announce_list: Option<Vec<(usize, String)>>,
    // The UNIX epoch timestamp of when this torrent was created
    pub creation_date: Option<u64>,
    // Free-form textual comments of the author
    pub comment: Option<String>,
    // Name and version of the program used to create the .torrent
    pub created_by: Option<String>,
    // The string encoding format used to generate the pieces part of the info dictionary in the
    // .torrent metafile
    pub encoding: Option<String>,
}

impl FromValue for SingleFile {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> where Self: Sized {
        let map = val.dict().ok_or("Single file not a dictionary".to_string())?;

        let file_name = map.get("name".as_bytes()).and_then(Value::bstring_utf8)
            .ok_or("Missing key: name".to_string())?;

        let length = map.get("length".as_bytes()).and_then(Value::integer)
            .map(|i| *i as usize).ok_or("Missing key: length".to_string())?;

        let md5sum = map.get("md5".as_bytes()).and_then(Value::bstring_utf8);

        Ok(SingleFile {
            file_name,
            length,
            md5sum,
        })
    }
}

impl FromValue for MultiFile {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> where Self: Sized {
        unimplemented!()
    }
}

impl FromValue for FileInfo {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> where Self: Sized {
        let map = val.dict().ok_or("File info not a dictionary".to_string())?;
        match (map.get("length".as_bytes()), map.get("files".as_bytes())) {
            (Some(_), None) => SingleFile::from_value(val).map(|f| FileInfo::Single(f)),
            (None, Some(_)) => MultiFile::from_value(val).map(|f| FileInfo::Multi(f)),
            _ => Err("Invalid dictionary".to_string())
        }
    }
}

impl FileInfo {
    /// Gets the total size requirements of the torrent in bytes
    pub fn size(&self) -> usize {
        match self {
            FileInfo::Single(s) => s.length,
            FileInfo::Multi(m) => m.files.iter().fold(0, |a, h| a + h.length)
        }
    }
}

impl FromValue for InfoDict {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> where Self: Sized {
        let map = val.dict().ok_or("Info not a dictionary".to_string())?;

        let piece_length = map.get("piece length".as_bytes()).and_then(Value::integer)
            .map(|i| *i as usize)
            .ok_or("Missing key: piece length".to_string())?;

        let pieces = map.get("pieces".as_bytes()).and_then(Value::bstring)
            .map(|bytes| bytes.chunks(20).map(|chunk| {
                chunk.iter()
                    .map(|byte| format!("{:02x?}", byte))
                    .collect::<Vec<_>>()
                    .join("")
            }).collect::<Vec<_>>()).ok_or("Missking key: pieces".to_string())?;

        let private = map.get("private".as_bytes()).and_then(Value::integer)
            .map_or(false, |i| *i == 1);

        let file_info = FileInfo::from_value(val)?;

        Ok(InfoDict {
            piece_length,
            pieces,
            private,
            file_info,
        })
    }
}

impl FromValue for MetaInfo {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> where Self: Sized {
        let map = val.dict().ok_or("Not a dictionary".to_string())?;

        let info_val = map.get("info".as_bytes()).ok_or("Missing key: info".to_string())?;
        let info_hash = sha1_hash(&info_val.clone().encode());
        let info = InfoDict::from_value(info_val)?;

        let announce = map.get("announce".as_bytes()).and_then(Value::bstring_utf8).ok_or("Missing key: announce".to_string())?;

        let announce_list = map.get("announce-list".as_bytes()).and_then(Value::list)
            .and_then(MetaInfo::interpret_announce_list);

        let creation_date = map.get("creation date".as_bytes()).and_then(Value::integer)
            .map(|i| *i as u64);

        let comment = map.get("comment".as_bytes()).and_then(Value::bstring_utf8);

        let created_by = map.get("created by".as_bytes()).and_then(Value::bstring_utf8);

        let encoding = map.get("encoding".as_bytes()).and_then(Value::bstring_utf8);

        Ok(MetaInfo {
            info_hash,
            info,
            announce,
            announce_list,
            creation_date,
            comment,
            created_by,
            encoding,
        })
    }
}

impl MetaInfo {
    fn interpret_announce_list(tiers: &Vec<Value>) -> Option<Vec<(usize, String)>> {
        let mut res = Vec::new();

        for (i, tier) in tiers.iter().enumerate() {
            if let Value::List(announces) = tier {
                for announce in announces {
                    if let Value::BString(bytes) = announce {
                        res.push((i, String::from_utf8(bytes.clone()).ok()?));
                    } else {
                        return None;
                    }
                }
            } else {
                return None;
            }
        }

        Some(res)
    }
}


fn sha1_hash(bytes: &[u8]) -> [u8; 20] {
    let mut res = [0u8; 20];
    let mut hasher = Sha1::new();
    hasher.input(bytes);
    hasher.result(&mut res);
    res
}