//! metainfo contains functions and types to parse the .torrent file
use boostencode::Value;
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


impl SingleFile {
    pub fn from_hashmap(map: &HashMap<Vec<u8>, Value>) -> Option<SingleFile> {
        let file_name = match map.get("name".as_bytes())?.to_owned() {
            // if it isn't utf8 then wtf
            Value::BString(bytes) => String::from_utf8(bytes).unwrap(),
            _ => return None,
        };

        let length = match map.get("length".as_bytes())? {
            Value::Integer(i) => *i as usize,
            _ => return None,
        };

        let md5sum = match map.get("md5".as_bytes()) {
            // not sure why we have to clone here
            Some(Value::BString(bytes)) => String::from_utf8(bytes.clone()).ok(),
            _ => None
        };

        Some(SingleFile {
            file_name,
            length,
            md5sum,
        })
    }
}

impl MultiFile {
    pub fn from_hashmap(_map: &HashMap<Vec<u8>, Value>) -> Option<MultiFile> {
        // TODO reconsider Multi/Single file structure (multi is collection of single?)
        unimplemented!()
    }
}

impl FileInfo {
    pub fn from_hashmap(map: &HashMap<Vec<u8>, Value>) -> Option<FileInfo> {
        match (map.get("length".as_bytes()), map.get("files".as_bytes())) {
            (Some(_), None) => SingleFile::from_hashmap(map).map(|f| FileInfo::Single(f)),
            (None, Some(_)) => MultiFile::from_hashmap(map).map(|f| FileInfo::Multi(f)),
            _ => None
        }
    }
}

impl InfoDict {
    pub fn from_hashmap(map: &HashMap<Vec<u8>, Value>) -> Option<InfoDict> {
        let piece_length = match map.get("piece length".as_bytes())? {
            Value::Integer(i) => *i as usize,
            _ => return None,
        };

        let pieces = match map.get("pieces".as_bytes())? {
            Value::BString(bytes) => bytes.chunks(20).map(|chunk| {
                chunk.iter()
                    .map(|byte| format!("{:02x?}", byte))
                    .collect::<Vec<_>>()
                    .join("")
            }).collect::<Vec<_>>(),
            _ => return None,
        };

        let private = match map.get("private".as_bytes()) {
            Some(Value::Integer(1)) => true,
            _ => false,
        };

        let file_info = FileInfo::from_hashmap(map)?;

        Some(InfoDict {
            piece_length,
            pieces,
            private,
            file_info,
        })
    }
}

impl MetaInfo {
    pub fn from_value(val: Value) -> Option<Self> {
        let map: HashMap<Vec<u8>, Value> = match val {
            Value::Dict(m) => m,
            _ => return None
        };

        // clone because we need the info dict in two places
        let info = match map.get("info".as_bytes())?.to_owned() {
            Value::Dict(info_dict) => info_dict,
            _ => return None
        };

        let info_hash = sha1_hash(Value::Dict(info.clone()).encode().as_ref());
        let info = InfoDict::from_hashmap(&info)?;

        let announce = match map.get("announce".as_bytes())? {
            Value::BString(bytes) => String::from_utf8(bytes.clone()).ok()?,
            _ => return None,
        };

        let announce_list = match map.get("announce-list".as_bytes()) {
            Some(Value::List(vals)) => MetaInfo::interpret_announce_list(vals),
            _ => None,
        };

        let creation_date = match map.get("creation date".as_bytes()) {
            Some(Value::Integer(i)) => Some(*i as u64),
            _ => None,
        };

        let comment = match map.get("comment".as_bytes()) {
            Some(Value::BString(bytes)) => String::from_utf8(bytes.clone()).ok(),
            _ => None,
        };

        let created_by = match map.get("created by".as_bytes()) {
            Some(Value::BString(bytes)) => String::from_utf8(bytes.clone()).ok(),
            _ => None,
        };

        let encoding = match map.get("encoding".as_bytes()) {
            Some(Value::BString(bytes)) => String::from_utf8(bytes.clone()).ok(),
            _ => None,
        };

        Some(MetaInfo {
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