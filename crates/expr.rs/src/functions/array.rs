use indexmap::IndexMap;
use crate::{bail, Environment, Value};

pub fn add_array_functions(env: &mut Environment) {
    env.add_function("all", |c| {
        if c.args.len() != 1 {
            bail!("all() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for value in a {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(false) = c.env.run(predicate.clone(), &ctx)? {
                    return Ok(false.into());
                }
            }
            Ok(true.into())
        } else {
            bail!("all() takes an array as the first argument");
        }
    });

    env.add_function("any", |c| {
        if c.args.len() != 1 {
            bail!("any() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for value in a {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(true) = c.env.run(predicate.clone(), &ctx)? {
                    return Ok(true.into());
                }
            }
            Ok(false.into())
        } else {
            bail!("any() takes an array as the first argument");
        }
    });

    env.add_function("one", |c| {
        if c.args.len() != 1 {
            bail!("one() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            let mut found = false;
            for value in a {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(true) = c.env.run(predicate.clone(), &ctx)? {
                    if found {
                        return Ok(false.into());
                    }
                    found = true;
                }
            }
            Ok(found.into())
        } else {
            bail!("one() takes an array as the first argument");
        }
    });

    env.add_function("none", |c| {
        if c.args.len() != 1 {
            bail!("none() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for value in a {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(true) = c.env.run(predicate.clone(), &ctx)? {
                    return Ok(false.into());
                }
            }
            Ok(true.into())
        } else {
            bail!("none() takes an array as the first argument");
        }
    });

    env.add_function("map", |c| {
        let mut result = Vec::new();
        if c.args.len() != 1 {
            bail!("map() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for value in a {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                result.push(c.env.run(predicate.clone(), &ctx)?);
            }
        } else {
            bail!("map() takes an array as the first argument");
        }
        Ok(result.into())
    });

    env.add_function("filter", |c| {
        let mut result = Vec::new();
        if c.args.len() != 1 {
            bail!("filter() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for value in a {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(true) = c.env.run(predicate.clone(), &ctx)? {
                    result.push(value.clone());
                }
            }
        } else {
            bail!("filter() takes an array as the first argument");
        }
        Ok(result.into())
    });

    env.add_function("find", |c| {
        if c.args.len() != 1 {
            bail!("find() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for value in a {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(true) = c.env.run(predicate.clone(), &ctx)? {
                    return Ok(value.clone());
                }
            }
            Ok(Value::Nil)
        } else {
            bail!("find() takes an array as the first argument");
        }
    });

    env.add_function("findIndex", |c| {
        if c.args.len() != 1 {
            bail!("findIndex() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for (i, value) in a.iter().enumerate() {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(true) = c.env.run(predicate.clone(), &ctx)? {
                    return Ok(i.into());
                }
            }
            Ok(Value::Number(-1))
        } else {
            bail!("findIndex() takes an array as the first argument");
        }
    });

    env.add_function("findLast", |c| {
        if c.args.len() != 1 {
            bail!("findLast() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for value in a.iter().rev() {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(true) = c.env.run(predicate.clone(), &ctx)? {
                    return Ok(value.clone());
                }
            }
            Ok(Value::Nil)
        } else {
            bail!("findLast() takes an array as the first argument");
        }
    });

    env.add_function("findLastIndex", |c| {
        if c.args.len() != 1 {
            bail!("findLastIndex() takes exactly one argument and a predicate");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            for (i, value) in a.iter().enumerate().rev() {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Value::Bool(true) = c.env.run(predicate.clone(), &ctx)? {
                    return Ok(i.into());
                }
            }
            Ok(Value::Number(-1))
        } else {
            bail!("findLastIndex() takes an array as the first argument");
        }
    });
    env.add_function("groupBy", |c| {
        if c.args.len() != 1 {
            bail!("groupBy() takes exactly two arguments");
        }
        if let (Value::Array(a), Some(predicate)) = (&c.args[0], c.predicate) {
            let mut groups = IndexMap::new();
            for value in a {
                let mut ctx = c.ctx.clone();
                ctx.insert("#".to_string(), value.clone());
                if let Some(key) = c.env.run(predicate.clone(), &ctx)?.as_string() {
                    groups.entry(key.to_string()).or_insert_with(Vec::new).push(value.clone());
                } else {
                    bail!("groupBy() predicate must return a string");
                }
            }
            Ok(Value::Map(groups.into_iter().map(|(k, group)| (k, group.into())).collect()))
        } else {
            bail!("groupBy() takes an array as the first argument and a predicate as the second argument");
        }
    });
}
