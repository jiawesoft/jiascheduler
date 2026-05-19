use crate::Rule;
use indexmap::IndexMap;
use log::trace;
use pest::iterators::{Pair, Pairs};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};

/// Represents a data value as input or output to an expr program
#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Value {
    Number(i64),
    Bool(bool),
    Float(f64),
    #[default]
    Nil,
    String(String),
    Array(Vec<Value>),
    Map(IndexMap<String, Value>),
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<i64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&IndexMap<String, Value>> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }
}

impl<K, V> FromIterator<(K, V)> for Value
where
    K: Into<String>, 
    V: Into<Value>,
{
    fn from_iter<I>(iter: I) -> Self
    where I: IntoIterator<Item = (K, V)> {
        Value::Map(iter.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
    }
}

impl AsRef<Value> for Value {
    fn as_ref(&self) -> &Value {
        self
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Number(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Number(n as i64)
    }
}

impl From<usize> for Value {
    fn from(n: usize) -> Self {
        Value::Number(n as i64)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self {
        s.to_string().into()
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl<V: Into<Value>> From<Vec<V>> for Value {
    fn from(a: Vec<V>) -> Self {
        Value::Array(a.into_iter().map(|v| v.into()).collect())
    }
}

impl From<IndexMap<String, Value>> for Value {
    fn from(m: IndexMap<String, Value>) -> Self {
        Value::Map(m)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Nil => write!(f, "nil"),
            Value::String(s) => write!(
                f,
                r#""{}""#,
                s.replace("\\", "\\\\")
                    .replace("\n", "\\n")
                    .replace("\r", "\\r")
                    .replace("\t", "\\t")
                    .replace("\"", "\\\"")
            ),
            Value::Array(a) => write!(
                f,
                "[{}]",
                a.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Value::Map(m) => write!(
                f,
                "{{{}}}",
                m.iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl From<Pairs<'_, Rule>> for Value {
    fn from(mut pairs: Pairs<Rule>) -> Self {
        pairs.next().unwrap().into()
    }
}

impl From<Pair<'_, Rule>> for Value {
    fn from(pair: Pair<Rule>) -> Self {
        trace!("{:?} = {}", &pair.as_rule(), pair.as_str());
        match pair.as_rule() {
            Rule::value => pair.into_inner().into(),
            Rule::nil => Value::Nil,
            Rule::bool => Value::Bool(pair.as_str().parse().unwrap()),
            Rule::int => Value::Number(pair.as_str().parse().unwrap()),
            Rule::decimal => Value::Float(pair.as_str().parse().unwrap()),
            Rule::string_multiline => pair.into_inner().as_str().into(),
            Rule::string => pair
                .into_inner()
                .as_str()
                .replace("\\\\", "\\")
                .replace("\\n", "\n")
                .replace("\\r", "\r")
                .replace("\\t", "\t")
                .replace("\\\"", "\"")
                .into(),
            // Rule::operation => {
            //     let mut pairs = pair.into_inner();
            //     let operator = pairs.next().unwrap().into();
            //     let left = Box::new(pairs.next().unwrap().into());
            //     let right = Box::new(pairs.next().unwrap().into());
            //     Node::Operation {
            //         operator,
            //         left,
            //         right,
            //     }
            // }
            rule => unreachable!("Unexpected rule: {rule:?} {}", pair.as_str()),
        }
    }
}
