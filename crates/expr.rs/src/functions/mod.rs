pub mod array;
pub mod string;
pub mod types;

use crate::Result;

use crate::ast::program::Program;
use crate::{bail, Context, Environment, Value};

pub type Function<'a> = Box<dyn Fn(ExprCall) -> Result<Value> + 'a + Sync + Send>;

pub struct ExprCall<'a, 'b> {
    pub ident: String,
    pub args: Vec<Value>,
    pub predicate: Option<Program>,
    pub ctx: &'a Context,
    pub env: &'a Environment<'b>,
}

impl Environment<'_> {
    pub fn eval_func(
        &self,
        ctx: &Context,
        ident: String,
        args: Vec<Value>,
        predicate: Option<Program>,
    ) -> Result<Value> {
        let call = ExprCall {
            ident,
            args,
            predicate,
            ctx,
            env: self,
        };
        if let Some(f) = self.functions.get(&call.ident) {
            f(call)
        } else {
            bail!("Unknown function: {}", call.ident)
        }
    }
}
