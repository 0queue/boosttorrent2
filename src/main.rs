use boostencode::Value;

mod boostencode;

fn main() {
    let val = Value::decode("d4:dictl5:hello5:worldi10ee3:sixi6ee".to_string().into_bytes().as_mut());
    let into: Value = "l4:spami3ee".to_string().into();

    println!("{:?}", val);
    println!("{:?}", into);

    println!("{}", std::str::from_utf8(val.encode().as_ref()).unwrap());
}