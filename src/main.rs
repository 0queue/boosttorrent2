#[macro_use]
extern crate derive_error;

use boostencode::Value;

mod boostencode;

fn main() {
    let val = Value::decode("d4:dictl5:hello5:worldi10ee3:sixi6ee".to_string().into_bytes().as_mut());//.unwrap();
    match val {
        Ok(val) => {
            println!("{:?}", val);

            println!("{}", std::str::from_utf8(val.encode().as_ref()).unwrap());
        }
        Err(e) =>
            println!("{}", e)
    }
}