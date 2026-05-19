use crate::ast::node::Node;
use crate::ast::program::Program;
use crate::eval::Environment;
use crate::functions::ExprCall;
use crate::{Context, Error, Result, Value};
use crate::{ExprPest, Rule};
use pest::Parser as PestParser;
use std::fmt;
use std::fmt::{Debug, Formatter};

/// Parse an expr program to be run later
pub fn compile(code: &str) -> Result<Program> {
    let pairs = ExprPest::parse(Rule::full, code).map_err(|e| Error::PestError(Box::new(e)))?;
    Ok(pairs.into())
}

/// Main struct for parsing and evaluating expr programs
///
/// Example:
///
/// ```
/// use expr::{Context, Parser};
/// let ctx = Context::from_iter([("foo", 1), ("bar", 2)]);
/// let p = Parser::new();
/// assert_eq!(p.eval("foo + bar", &ctx).unwrap().to_string(), "3");
/// ```
#[deprecated(note = "Use `compile()` and `Environment` instead")]
#[derive(Default)]
pub struct Parser<'a> {
    env: Environment<'a>,
}

#[allow(deprecated)]
impl Debug for Parser<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("ExprParser").finish()
    }
}

#[allow(deprecated)]
impl<'a> Parser<'a> {
    /// Create a new parser with the default environment
    pub fn new() -> Self {
        Parser {
            env: Environment::new(),
        }
    }

    /// Add a function for expr programs to call
    ///
    /// Example:
    /// ```
    /// use std::collections::HashMap;
    /// use expr::{Context, Parser, Value};
    ///
    /// let mut p = Parser::new();
    /// let ctx = Context::default();
    /// p.add_function("add", |c| {
    ///   let mut sum = 0;
    ///     for arg in c.args {
    ///       if let Value::Number(n) = arg {
    ///         sum += n;
    ///        } else {
    ///          panic!("Invalid argument: {arg:?}");
    ///        }
    ///     }
    ///   Ok(sum.into())
    /// });
    /// assert_eq!(p.eval("add(1, 2, 3)", &ctx).unwrap().to_string(), "6");
    /// ```
    pub fn add_function<F>(&mut self, name: &str, f: F)
    where
        F: Fn(ExprCall) -> Result<Value> + 'a + Sync + Send,
    {
        self.env.add_function(name, Box::new(f));
    }

    /// Parse an expr program to be run later
    pub fn compile(&self, code: &str) -> Result<Program> {
        compile(code)
    }

    /// Run a compiled expr program
    pub fn run(&self, program: Program, ctx: &Context) -> Result<Value> {
        self.env.run(program, ctx)
    }

    /// Compile and run an expr program in one step
    ///
    /// Example:
    /// ```
    /// use std::collections::HashMap;
    /// use expr::{Context, Parser};
    /// let p = Parser::default();
    /// let ctx = Context::default();
    /// assert_eq!(p.eval("1 + 2", &ctx).unwrap().to_string(), "3");
    /// ```
    pub fn eval(&self, code: &str, ctx: &Context) -> Result<Value> {
        self.env.eval(code, ctx)
    }

    pub fn eval_expr(&self, ctx: &Context, node: Node) -> Result<Value> {
        self.env.eval_expr(ctx, node)
    }
}
