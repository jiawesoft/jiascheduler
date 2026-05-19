use crate::{bail, Environment, Value};

pub fn add_string_functions(env: &mut Environment) {
    env.add_function("trim", |c| {
        if c.args.len() != 1 && c.args.len() != 2 {
            bail!("trim() takes one or two arguments");
        }
        if let (Value::String(s), None) = (&c.args[0], c.args.get(1)) {
            Ok(s.trim().into())
        } else if let (Value::String(s), Some(Value::String(chars))) = (&c.args[0], c.args.get(1)) {
            Ok(s.trim_matches(|c| chars.contains(c)).into())
        } else {
            bail!("trim() takes a string as the first argument and an optional string of characters to trim");
        }
    });

    env.add_function("trimPrefix", |c| {
        if let (Value::String(s), Value::String(prefix)) = (&c.args[0], &c.args[1]) {
            Ok(s.strip_prefix(prefix).unwrap_or(s).into())
        } else {
            bail!("trimPrefix() takes a string as the first argument and a string to trim as the second argument");
        }
    });

    env.add_function("trimSuffix", |c| {
        if let (Value::String(s), Value::String(suffix)) = (&c.args[0], &c.args[1]) {
            Ok(s.strip_suffix(suffix).unwrap_or(s).into())
        } else {
            bail!("trimSuffix() takes a string as the first argument and a string to trim as the second argument");
        }
    });

    env.add_function("upper", |c| {
        if c.args.len() != 1 {
            bail!("upper() takes one argument");
        }
        if let Value::String(s) = &c.args[0] {
            Ok(s.to_uppercase().into())
        } else {
            bail!("upper() takes a string as the first argument");
        }
    });

    env.add_function("string", |c| {
        if c.args.len() != 1 {
            bail!("upper() takes one argument");
        }

        let v = match &c.args[0] {
            Value::Number(v) => format!("{v}").to_string(),
            Value::Bool(v) => format!("{v}").to_string(),
            Value::Float(v) => format!("{v}").to_string(),
            Value::Nil => format!("<nil>").to_string(),
            Value::String(v) => v.to_string(),
            Value::Array(values) => format!("{:?}", values),
            Value::Map(index_map) => format!("{:?}", index_map),
        };
        Ok(v.into())
    });

    env.add_function("lower", |c| {
        if c.args.len() != 1 {
            bail!("lower() takes one argument");
        }
        if let Value::String(s) = &c.args[0] {
            Ok(s.to_lowercase().into())
        } else {
            bail!("lower() takes a string as the first argument");
        }
    });

    env.add_function("split", |c| {
        if let (Value::String(s), Value::String(sep), None) =
            (&c.args[0], &c.args[1], c.args.get(2))
        {
            Ok(s.split(sep).map(Value::from).collect::<Vec<_>>().into())
        } else if let (Value::String(s), Value::String(sep), Some(Value::Number(n))) =
            (&c.args[0], &c.args[1], c.args.get(2))
        {
            Ok(s.splitn(*n as usize, sep)
                .map(Value::from)
                .collect::<Vec<_>>()
                .into())
        } else {
            bail!(
                "split() takes a string as the first argument and a string as the second argument"
            );
        }
    });

    env.add_function("splitAfter", |c| {
        if let (Value::String(s), Value::String(sep), None) = (&c.args[0], &c.args[1], c.args.get(2)) {
            Ok(s.split_inclusive(sep).map(Value::from).collect::<Vec<_>>().into())
        } else if let (Value::String(s), Value::String(sep), Some(Value::Number(n))) = (&c.args[0], &c.args[1], c.args.get(2)) {
            let mut arr = s.split_inclusive(sep).take(*n as usize - 1).map(|s| s.to_string()).collect::<Vec<_>>();
            arr.push(s.split_inclusive(sep).skip(*n as usize - 1).collect::<Vec<_>>().join(""));
            Ok(arr.into())
        } else {
            bail!("splitAfter() takes a string as the first argument and a string as the second argument");
        }
    });

    env.add_function("replace", |c| {
        if let (Value::String(s), Value::String(from), Value::String(to)) =
            (&c.args[0], &c.args[1], &c.args[2])
        {
            Ok(s.replace(from, to).into())
        } else {
            bail!("replace() takes a string as the first argument and two strings to replace");
        }
    });

    env.add_function("repeat", |c| {
        if let (Value::String(s), Value::Number(n)) = (&c.args[0], &c.args[1]) {
            Ok(s.repeat(*n as usize + 1).into())
        } else {
            bail!(
                "repeat() takes a string as the first argument and a number as the second argument"
            );
        }
    });

    env.add_function("indexOf", |c| {
        if let (Value::String(s), Value::String(sub)) = (&c.args[0], &c.args[1]) {
            Ok(s.find(sub).map(|i| i as i64).unwrap_or(-1).into())
        } else {
            bail!("indexOf() takes a string as the first argument and a string to search for as the second argument");
        }
    });

    env.add_function("lastIndexOf", |c| {
        if let (Value::String(s), Value::String(sub)) = (&c.args[0], &c.args[1]) {
            Ok(s.rfind(sub).map(|i| i as i64).unwrap_or(-1).into())
        } else {
            bail!("lastIndexOf() takes a string as the first argument and a string to search for as the second argument");
        }
    });

    env.add_function("hasPrefix", |c| {
        if let (Value::String(s), Value::String(prefix)) = (&c.args[0], &c.args[1]) {
            Ok(s.starts_with(prefix).into())
        } else {
            bail!("hasPrefix() takes a string as the first argument and a string to search for as the second argument");
        }
    });

    env.add_function("hasSuffix", |c| {
        if let (Value::String(s), Value::String(suffix)) = (&c.args[0], &c.args[1]) {
            Ok(s.ends_with(suffix).into())
        } else {
            bail!("hasSuffix() takes a string as the first argument and a string to search for as the second argument");
        }
    });
}
