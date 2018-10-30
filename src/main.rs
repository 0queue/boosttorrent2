extern crate clap;
extern crate crypto;
extern crate derive_error;
extern crate hyper;
extern crate maplit;

use boostencode::Value;
use clap::App;
use clap::load_yaml;
use std::fs::File;
use std::io::Read;

mod boostencode;
mod metainfo;

fn main() {
    // unwrapping is fine we know it's valid
    let val = Value::decode("d4:dictl5:hello5:worldi10ee3:sixi6ee".to_string().into_bytes().as_mut()).unwrap();
    println!("{:?}", val);
    println!("{}", std::str::from_utf8(val.encode().as_ref()).unwrap());

    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    if matches.is_present("garbage-mode") {
        println!("Garbage mode activated");
    }

    if matches.is_present("torrent-file") {
        let string = matches.value_of("torrent-file").unwrap();
        let mut f = File::open(string).expect("file not found");
        let mut contents = Vec::new();
        f.read_to_end(&mut contents).expect("error reading file");
        let val = Value::decode(contents.as_ref()).unwrap();
        println!("{}", val);

        let metainfo = metainfo::MetaInfo::from_value(val).unwrap();
        println!("{:?}", metainfo)
    }
}