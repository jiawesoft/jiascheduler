use crate::Value;
use indexmap::IndexMap;
use std::fmt::Display;

#[derive(Debug, Clone, Default)]
pub struct Context(pub(crate) IndexMap<String, Value>);

impl Context {
    pub fn insert<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<Value>,
    {
        self.0.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
}

impl<S: Display, T: Into<Value>> FromIterator<(S, T)> for Context {
    fn from_iter<I: IntoIterator<Item=(S, T)>>(iter: I) -> Self {
        let mut ctx = Self::default();
        for (k, v) in iter {
            ctx.insert(k.to_string(), v);
        }
        ctx
    }
}
