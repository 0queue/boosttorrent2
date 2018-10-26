extern crate derive_error;

use boostencode::Value;

mod boostencode;
mod metainfo;

fn main() {
    // unwrapping is fine we know it's valid
    let val = Value::decode("d4:dictl5:hello5:worldi10ee3:sixi6ee".to_string().into_bytes().as_mut()).unwrap();
    println!("{:?}", val);
    println!("{}", std::str::from_utf8(val.encode().as_ref()).unwrap());
}