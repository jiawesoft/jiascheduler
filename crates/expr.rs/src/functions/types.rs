use crate::{bail, Environment, Value};

pub fn add_types_functions(env: &mut Environment) {
    env.add_function("float", |c| {
        if c.args.len() != 1 {
            bail!("float() takes one argument");
        }

        match &c.args[0] {
            Value::Number(v) => {
                let v = *v;
                Ok((v as f64).into())
            }
            Value::Float(v) => Ok((*v).into()),
            Value::String(v) => {
                let v = v.parse::<f64>();
                match v {
                    Ok(v) => Ok(v.into()),
                    Err(e) => bail!("{e}"),
                }
            }
            _ => bail!("upper() takes a string, number as the first argument"),
        }
    });
}
