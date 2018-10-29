//! metainfo contains functions and types to parse the .torrent file
use boostencode::{DecodeError, Value};
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use derive_error::Error;
use std::collections::HashMap;
use std::io::{Error, Read};
use std::str::FromStr;


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
    // An optional list of more trackers
    pub announce_list: Option<Vec<String>>,
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

#[derive(Debug, Error)]
pub enum TorrentParseError {
    /// Could not read from the torrent file
    IOError(Error),
    /// Could not bdecode the torrent file
    DecodeError(DecodeError),
    /// Torrent file was not in the expected format
    FormatError,

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

        // TODO update announce list based on http://bittorrent.org/beps/bep_0012.html
        // currently is a flat list, should be tiered
        let announce_list: Option<Vec<String>> = None;

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
            announce_list: None,
            creation_date,
            comment,
            created_by,
            encoding,
        })
    }

    /// Attempts to parse the given Reader as a torrent file
    pub fn from_torrent<T: Read>(torrent: &mut T) -> Result<Self, TorrentParseError> {
        // Read the bytes and bdecode them
        let bytes = torrent.bytes().collect::<Result<Vec<u8>, Error>>()?;
        let info_hash = Self::get_info_hash(&bytes)?;
        let parsed = Value::decode(&bytes)?;
        let mut toplevel_dict = match parsed {
            Value::Dict(d) => Ok(d),
            _ => Err(TorrentParseError::FormatError)
        }?;

        let announce = toplevel_dict.remove("announce".as_bytes())
            .ok_or(TorrentParseError::FormatError)
            .and_then(|val| {
                if let Value::BString(bs) = val {
                    String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            })?;
        let announce_list = Self::transpose(toplevel_dict.remove("announce-list".as_bytes())
            .map(|val| {
                if let Value::List(l) = val {
                    l.into_iter().map(|val| {
                        if let Value::BString(bs) = val {
                            String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                        } else {
                            Err(TorrentParseError::FormatError)
                        }
                    }).collect::<Result<Vec<String>, TorrentParseError>>()
                } else {
                    Err(TorrentParseError::FormatError)
                }
            }))?;
        let creation_date = Self::transpose(toplevel_dict.remove("creation date".as_bytes())
            .map(|val| {
                if let Value::Integer(i) = val {
                    Ok(i as u64)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            }))?;
        let comment = Self::transpose(toplevel_dict.remove("comment".as_bytes())
            .map(|val| {
                if let Value::BString(bs) = val {
                    String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            }))?;
        let created_by = Self::transpose(toplevel_dict.remove("created by".as_bytes())
            .map(|val| {
                if let Value::BString(bs) = val {
                    String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            }))?;
        let encoding = Self::transpose(toplevel_dict.remove("encoding".as_bytes())
            .map(|val| {
                if let Value::BString(bs) = val {
                    String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            }))?;
        let info_dict = toplevel_dict.remove("info".as_bytes())
            .ok_or(TorrentParseError::FormatError)
            .and_then(|val| {
                if let Value::Dict(d) = val {
                    Ok(d)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            })?;
        let info = Self::parse_info_dict(info_dict)?;
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

    fn get_info_hash(bytes: &[u8]) -> Result<[u8; 20], TorrentParseError> {
        let res = bytes.windows(6)
            .enumerate()
            .find(|(_, chunk)| {
                let chunk_str = std::str::from_utf8(chunk);
                match chunk_str {
                    Ok(str) => str == "4:info",
                    _ => false
                }
            })
            .map(|(idx, _)| idx + 6)
            .map(|start_idx| {
                let mut i = start_idx;
                let mut depth = 0;
                while i < bytes.len() {
                    match bytes[i] as char {
                        // skip ints
                        'i' => {
                            while bytes[i] as char != 'e' {
                                i += 1;
                            };
                            i += 1;
                        }
                        'd' | 'l' => {
                            i += 1;
                            depth += 1;
                        }
                        // skip strings
                        '0'...'9' => {
                            let mut num = String::new();
                            while i < bytes.len() && bytes[i] as char >= '0' && bytes[i] as char <= '9' {
                                num.push(bytes[i] as char);
                                i += 1
                            }

                            i += 1 + usize::from_str(num.as_str()).map_err(|_| TorrentParseError::FormatError)?;
                        }
                        'e' => {
                            depth -= 1;
                            i += 1;
                            if depth == 0 {
                                return Ok((start_idx, i));
                            }
                        }
                        _ => return Err(TorrentParseError::FormatError)
                    }
                }
                Err(TorrentParseError::FormatError)
            })
            .map(|indexes| {
                indexes.map(|(start_idx, end_idx)| {
                    let mut hasher = Sha1::new();
                    hasher.input(&bytes[start_idx..end_idx]);
                    let mut res: [u8; 20] = [0; 20];
                    hasher.result(&mut res);
                    res
                })
            });

        match res {
            Some(Ok(hash)) => Ok(hash),
            Some(err) => err,
            None => Err(TorrentParseError::FormatError)
        }
    }

    /// Transposes an Option of a Result into a Result of an Option.
    /// None will be mapped to Ok(None). Some(Ok(_)) and Some(Err(_)) will be mapped to Ok(Some(_)) and Err(_).
    /// Using this because Option::transpose is not stable for some reason
    fn transpose<T, E>(opt: Option<Result<T, E>>) -> Result<Option<T>, E> {
        match opt {
            Some(res) => res.map(|val| Some(val)),
            None => Ok(None)
        }
    }

    fn parse_info_dict(mut info_dict: HashMap<Vec<u8>, Value>) -> Result<InfoDict, TorrentParseError> {
        let piece_length = info_dict.remove("piece length".as_bytes())
            .ok_or(TorrentParseError::FormatError)
            .and_then(|val| {
                if let Value::Integer(i) = val {
                    Ok(i as usize)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            })?;
        let pieces = info_dict.remove("pieces".as_bytes())
            .ok_or(TorrentParseError::FormatError)
            .and_then(|val| {
                if let Value::BString(bs) = val {
                    Ok(bs)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            }).
            map(|bs| {
                // bs is a concatenation of 20 byte sha1 hashes, so convert it to a list of hex strings
                bs.chunks(20)
                    .map(|hash_bytes| {
                        hash_bytes.into_iter().fold(String::new(), |mut a, h| {
                            a.push_str(&format!("{:02x?}", h));
                            a
                        })
                    })
                    .collect::<Vec<String>>()
            })?;
        let private = info_dict.remove("private".as_bytes())
            .map_or(false, |val| {
                if let Value::Integer(i) = val {
                    i == 1
                } else {
                    false
                }
            });
        let file_info = Self::parse_file_info(info_dict)?;
        Ok(InfoDict {
            piece_length,
            pieces,
            private,
            file_info,
        })
    }

    fn parse_file_info(mut info_dict: HashMap<Vec<u8>, Value>) -> Result<FileInfo, TorrentParseError> {
        let file_name = info_dict.remove("name".as_bytes())
            .ok_or(TorrentParseError::FormatError)
            .and_then(|val| {
                if let Value::BString(bs) = val {
                    String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                } else {
                    Err(TorrentParseError::FormatError)
                }
            })?;
        let files = info_dict.remove("files".as_bytes())
            .map(|val| {
                // info_dict has "files", means this is multi file, so parse out the multiple files
                if let Value::List(l) = val {
                    l.into_iter().map(|val| {
                        if let Value::Dict(mut d) = val {
                            let length = d.remove("length".as_bytes())
                                .ok_or(TorrentParseError::FormatError)
                                .and_then(|val| {
                                    if let Value::Integer(i) = val {
                                        Ok(i as usize)
                                    } else {
                                        Err(TorrentParseError::FormatError)
                                    }
                                })?;
                            let file_name = d.remove("path".as_bytes())
                                .ok_or(TorrentParseError::FormatError)
                                .and_then(|val| {
                                    if let Value::BString(bs) = val {
                                        String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                                    } else {
                                        Err(TorrentParseError::FormatError)
                                    }
                                })?;
                            let md5 = d.remove("md5".as_bytes())
                                .map(|val| {
                                    if let Value::BString(bs) = val {
                                        String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                                    } else {
                                        Err(TorrentParseError::FormatError)
                                    }
                                });
                            let md5sum = Self::transpose(md5)?;

                            Ok(SingleFile {
                                file_name,
                                length,
                                md5sum,
                            })
                        } else {
                            Err(TorrentParseError::FormatError)
                        }
                    }).collect::<Result<Vec<SingleFile>, TorrentParseError>>()
                } else {
                    Err(TorrentParseError::FormatError)
                }
            });
        let file_info = match files {
            Some(files) => FileInfo::Multi(MultiFile {
                root_dir_name: file_name,
                files: files?,
            }),
            None => {
                let length = info_dict.remove("length".as_bytes())
                    .ok_or(TorrentParseError::FormatError)
                    .and_then(|val| {
                        if let Value::Integer(i) = val {
                            Ok(i as usize)
                        } else {
                            Err(TorrentParseError::FormatError)
                        }
                    })?;
                let md5 = info_dict.remove("md5".as_bytes())
                    .map(|val| {
                        if let Value::BString(bs) = val {
                            String::from_utf8(bs).map_err(|_| TorrentParseError::FormatError)
                        } else {
                            Err(TorrentParseError::FormatError)
                        }
                    });
                let md5sum = Self::transpose(md5)?;

                FileInfo::Single(SingleFile {
                    file_name,
                    length,
                    md5sum,
                })
            }
        };

        Ok(file_info)
    }
}

fn sha1_hash(bytes: &[u8]) -> [u8; 20] {
    let mut res = [0u8; 20];
    let mut hasher = Sha1::new();
    hasher.input(bytes);
    hasher.result(&mut res);
    res
}