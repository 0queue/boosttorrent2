use serde::ser::{self, Serialize};
use std::fmt::Display;
use super::{DecodeError, Result};

#[cfg(test)]
mod test;


pub struct Serializer {
    output: Vec<u8>
}

pub fn to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut serializer = Serializer { output: Vec::new() };
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    let mut serializer = Serializer{ output: Vec::new() };
    value.serialize(&mut serializer)?;
    String::from_utf8(serializer.output).map_err(|_| DecodeError::InvalidString)
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = DecodeError;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.serialize_i64(if v { 1 } else { 0 })
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        let mut bytes = Vec::from(format!("i{}e", v).as_bytes());
        self.output.append(&mut bytes);
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        let mut bytes = Vec::from(format!("i{}e", v).as_bytes());
        self.output.append(&mut bytes);
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        Err(DecodeError::InvalidInteger)
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        Err(DecodeError::InvalidInteger)
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.serialize_bytes(v.as_ref())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        let len = format!("{}:", v.len());
        self.output.append(&mut Vec::from(len.as_bytes()));
        self.output.append(&mut Vec::from(v));
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<()> where
        T: Serialize {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        // Nothing to do
        Ok(())
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(self, name: &'static str, variant_index: u32, variant: &'static str) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized>(self, name: &'static str, value: &T) -> Result<()> where
        T: Serialize {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(self, name: &'static str, variant_index: u32, variant: &'static str, value: &T) -> Result<()> where
        T: Serialize {
        unimplemented!() // not sure what to do
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.output.push('l' as u8);
        Ok(self)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(self, name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeTupleVariant> {
        unimplemented!() // what to do what to do
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        self.output.push('d' as u8);
        Ok(self)
    }

    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(self, name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeStructVariant> {
        unimplemented!() // what to do what to do
    }

    fn collect_str<T: ?Sized>(self, value: &T) -> Result<()> where
        T: Display {
        unimplemented!() // doesn't show up in docs???
    }
}

impl<'a> ser::SerializeSeq for &'a mut Serializer {
    type Ok = ();
    type Error = DecodeError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()> where
        T: Serialize {
        value.serialize(&mut **self)
    }


    fn end(self) -> Result<()> {
        self.output.push('e' as u8);
        Ok(())
    }
}

impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = ();
    type Error = DecodeError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()> where
        T: Serialize {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.output.push('e' as u8);
        Ok(())
    }
}

impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();
    type Error = DecodeError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()> where
        T: Serialize {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.output.push('e' as u8);
        Ok(())
    }
}

// don't know, blah
impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();
    type Error = DecodeError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()> where
        T: Serialize {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        unimplemented!()
    }
}

impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = ();
    type Error = DecodeError;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<()> where
        T: Serialize {
        // TODO can produce invalid bencoding if key is not bstring!!
        // a fix would be to create a KeySerializer and use that instead of self
        key.serialize(&mut **self)
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<()> where
        T: Serialize {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.output.push('e' as u8);
        Ok(())
    }
}

// TODO somehow guarantee ordered keys?
// serde seems to iterate in alpha order over fields but
// I'd like to be sure
impl<'a> ser::SerializeStruct for &'a mut Serializer {
    type Ok = ();
    type Error = DecodeError;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()> where
        T: Serialize {
        key.serialize(&mut **self)?;
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.output.push('e' as u8);
        Ok(())
    }
}

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = ();
    type Error = DecodeError;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()> where
        T: Serialize {
        key.serialize(&mut **self)?;
        value.serialize(&mut **self)
    }


    fn end(self) -> Result<()> {
        self.output.push('e' as u8);
        Ok(())
    }
}