extern crate clap;
extern crate derive_error;

use boostencode::Value;
use clap::App;
use clap::load_yaml;

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

    if matches.is_present("bencoded-string") {
        let string = matches.value_of("bencoded-string").unwrap();
        println!("{:?}", Value::decode(string.as_ref()));
    }
}