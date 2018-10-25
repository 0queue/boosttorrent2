//! metainfo contains functions and types to parse the .torrent file

pub struct SingleFile {
        // Full path of the file from the root
        file_name: String,
        // File size
        length: usize,
        // MD5 Sum of the entire file
        md5sum: Option<String>,
    }
pub struct MultiFile {
        // Name of the root directory of the torrent
        root_dir_name: String,
        // A list of all files in this torrent
        files: Vec<SingleFile>,
}
pub enum FileInfo {
    Single(SingleFile),
    Multi(MultiFile),
}

pub struct InfoDict {
    // The number of bytes in each piece
    piece_length: usize,
    // The SHA1 hashes of each piece
    pieces: Vec<String>,
    // If true, only publish presence via trackers and not directly to peers
    private: bool,
    // Information about the file(s) to download
    file_info: FileInfo,
}

pub struct MetaInfo {
    // Information about the file to be downloaded
    info: InfoDict,
    // The url for the tracker
    announce: String,
    // An optional list of more trackers
    announce_list: Option<Vec<String>>,
    // The UNIX epoch timestamp of when this torrent was created
    creation_date: Option<uint64>,
    // Free-form textual comments of the author
    comment: Option<String>,
    // Name and version of the program used to create the .torrent
    created_by: Option<String>,
    // The string encoding format used to generate the pieces part of the info dictionary in the
    // .torrent metafile
    encoding: Option<String>,
}