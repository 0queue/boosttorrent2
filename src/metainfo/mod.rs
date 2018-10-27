//! metainfo contains functions and types to parse the .torrent file
use boostencode::{DecodeError, Value};
use derive_error::Error;
use std::collections::HashMap;
use std::io::{Error, Read};

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

impl MetaInfo {
    /// Attempts to parse the given Reader as a torrent file
    pub fn from_torrent<T: Read>(torrent: &mut T) -> Result<Self, TorrentParseError> {
        // Read the bytes and bdecode them
        let bytes = torrent.bytes().collect::<Result<Vec<u8>, Error>>()?;
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
            info,
            announce,
            announce_list,
            creation_date,
            comment,
            created_by,
            encoding,
        })
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