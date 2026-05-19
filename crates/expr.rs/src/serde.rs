use crate::{Value, Error};
use indexmap::IndexMap;
use serde::Serialize;
use serde::de::{Deserializer, Visitor, DeserializeOwned, IntoDeserializer};
use serde::de::value::{SeqDeserializer, MapDeserializer};
use serde::ser::{Serializer};

pub fn to_value<T: Serialize + ?Sized>(t: &T) -> Result<Value, Error> {
    t.serialize(ValueSerializer {})
}

pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T, Error> {
    T::deserialize(value.into_deserializer())
}

impl serde::de::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::DeserializeError(msg.to_string())
    }
}

impl serde::ser::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::SerializeError(msg.to_string())
    }
}

#[derive(Debug)]
pub struct ValueDeserializer {
    value: Value,
}

impl<'de> Deserializer<'de> for ValueDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where V: Visitor<'de> {
        match self.value {
            Value::Number(n) => visitor.visit_i64(n),
            Value::Float(f) => visitor.visit_f64(f),
            Value::Bool(b) => visitor.visit_bool(b),
            Value::String(s) => visitor.visit_string(s),
            Value::Array(a) => visitor.visit_seq(SeqDeserializer::new(a.into_iter())),
            Value::Map(m) => visitor.visit_map(MapDeserializer::new(m.into_iter())),
            Value::Nil => visitor.visit_unit(),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de> IntoDeserializer<'de, Error> for Value {
    type Deserializer = ValueDeserializer;
    fn into_deserializer(self) -> Self::Deserializer {
        ValueDeserializer { value: self }
    }
}

#[derive(Debug)]
pub struct ValueSerializer {}

macro_rules! serialize_fn {
    ($name:ident, $type:ty, $cast_type:ty) => {
        #[inline]
        fn $name(self, v: $type) -> Result<Self::Ok, Self::Error> {
            Ok(Value::from(<$cast_type>::try_from(v).map_err(|e|
                Error::SerializeError(e.to_string())
            )?))
        }
    };
}

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SerializeSeq;
    type SerializeTuple = SerializeSeq;
    type SerializeTupleStruct = SerializeSeq;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = SerializeStructVariant;

    serialize_fn!(serialize_bool, bool, bool);
    serialize_fn!(serialize_i8, i8, i64);
    serialize_fn!(serialize_i16, i16, i64);
    serialize_fn!(serialize_i32, i32, i64);
    serialize_fn!(serialize_i64, i64, i64);
    serialize_fn!(serialize_i128, i128, i64);
    serialize_fn!(serialize_u8, u8, i64);
    serialize_fn!(serialize_u16, u16, i64);
    serialize_fn!(serialize_u32, u32, i64);
    serialize_fn!(serialize_u64, u64, i64);
    serialize_fn!(serialize_u128, u128, i64);
    serialize_fn!(serialize_f32, f32, f64);
    serialize_fn!(serialize_f64, f64, f64);
    serialize_fn!(serialize_str, &str, &str);

    #[inline]
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&v.to_string())
    }

    #[inline]
    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Error::SerializeError("Converting bytes to expr::Value is not supported".to_string()))
    }

    // An absent optional is converted to Value::Nil.
    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    // A present optional is converted to the contained value
    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + Serialize {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Nil)
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    // Unit variants convert to a string containing their name.
    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    // Unit structs convert to their contents.
    #[inline]
    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    // Newtype variant (along with other variant serialization methods)
    // refers exclusively to the "externally tagged" enum representation, so
    // convert this to an expr map of the form `{ NAME: VALUE }`.
    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Ok(Value::from_iter([(
            variant,
            value.serialize(self)?,
        )]))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SerializeSeq::new())
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(SerializeTupleVariant::new(variant))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(SerializeMap::new())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(SerializeStructVariant::new(variant))
    }
}

#[doc(hidden)]
pub struct SerializeSeq {
    vec: Vec<Value>,
}

impl SerializeSeq {
    pub fn new() -> Self {
        Self {
            vec: Vec::new(),
        }
    }
}

impl serde::ser::SerializeSeq for SerializeSeq {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + Serialize,
    {
        self.vec.push(value.serialize(ValueSerializer {})?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Array(self.vec))
    }
}

// SerializeTuple implementation delegates to SerializeSeq
impl serde::ser::SerializeTuple for SerializeSeq {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

// SerializeTupleStruct implementation delegates to SerializeSeq
impl serde::ser::SerializeTupleStruct for SerializeSeq {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + Serialize,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

// Tuple variant serialization is similar to seq, but wraps output in a unit
// map of the form `{ VARIANT_NAME: [...] }`.
#[doc(hidden)]
pub struct SerializeTupleVariant {
    name: String,
    vec: Vec<Value>,
}

impl SerializeTupleVariant {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            vec: Vec::new(),
        }
    }
}

impl serde::ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + Serialize,
    {
        self.vec.push(value.serialize(ValueSerializer {})?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::from_iter([(
            self.name,
            Value::Array(self.vec),
        )]))
    }
}

#[doc(hidden)]
pub struct SerializeMap {
    map: IndexMap<String, Value>,
    next_key: Option<String>,
}

impl SerializeMap {
    pub fn new() -> Self {
        Self {
            map: IndexMap::new(),
            next_key: None,
        }
    }
}

impl serde::ser::SerializeMap for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where T: ?Sized + Serialize,
    {
        match key.serialize(ValueSerializer {})? {
            Value::String(s) => {
                self.next_key = Some(s);
                Ok(())
            }
            _ => Err(Error::SerializeError("key must be a string".to_string())),
        }
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + Serialize,
    {
        let key = self.next_key.take().expect("serialize_value called before serialize_key");
        self.map.insert(key, value.serialize(ValueSerializer {})?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Map(self.map))
    }
}

// SerializeStruct implementation delegates to SerializeMap
impl serde::ser::SerializeStruct for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + Serialize,
    {
        serde::ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeMap::end(self)
    }
}

// Struct variant serialization is similar to map, but wraps output in a unit
// map of the form `{ VARIANT_NAME: {...} }`.
#[doc(hidden)]
pub struct SerializeStructVariant {
    name: String,
    map: IndexMap<String, Value>,
}

impl SerializeStructVariant {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            map: IndexMap::new(),
        }
    }
}

impl serde::ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where T: ?Sized + Serialize,
    {
        self.map.insert(key.to_owned(), value.serialize(ValueSerializer {})?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::from_iter([(
            self.name,
            Value::Map(self.map),
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
    struct Foo {
        a: String,
        b: i32,
        c: Bar,
        d: Vec<HashMap<String, String>>,
    }

    #[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
    struct Bar {
        x: f32,
        y: f32,
    }

    fn test_struct() -> Foo {
        Foo {
            a: "hello".to_string(),
            b: 123,
            c: Bar {
                x: 1.0,
                y: 2.0,
            },
            d: vec![
                HashMap::from([("j".to_string(), "k".to_string())]),
            ],
        }
    }

    fn test_value() -> Value {
        value!({
            "a": "hello",
            "b": 123,
            "c": {
                "x": 1.0,
                "y": 2.0,
            },
            "d": [{
                "j": "k",
            }],
        })
    }

    fn test_json() -> String {
        let raw = r#"{
            "a": "hello",
            "b": 123,
            "c": {
                "x": 1.0,
                "y": 2.0
            },
            "d": [{
                "j": "k"
            }]
        }"#;
        let val: serde_json::value::Value = serde_json::from_str(&raw).unwrap();
        serde_json::to_string_pretty(&val).unwrap()
    }

    #[test]
    fn serialize() {
        let val = test_value();
        assert_eq!(serde_json::to_string_pretty(&val).unwrap(), test_json());
    }

    #[test]
    fn deserialize() {
        let json = test_json();
        assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), test_value());
    }

    #[test]
    fn convert_to_value() {
        let val = to_value(&test_struct()).unwrap();
        assert_eq!(val, test_value());
    }

    #[test]
    fn convert_from_value() {
        let struct_val: Foo = from_value(test_value()).unwrap();
        assert_eq!(struct_val, test_struct());
    }
}