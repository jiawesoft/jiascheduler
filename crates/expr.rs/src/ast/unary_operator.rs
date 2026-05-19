use crate::ast::node::Node;
use crate::Rule;
use crate::Value::Bool;
use crate::{bail, Result};
use crate::{Context, Environment, Value};
use log::trace;
use pest::iterators::Pair;
use std::str::FromStr;

#[derive(Debug, Clone, strum::EnumString)]
pub enum UnaryOperator {
    #[strum(serialize = "!")]
    Not,
    #[strum(serialize = "+")]
    Positive,
    #[strum(serialize = "-")]
    Negative,
}

impl From<Pair<'_, Rule>> for UnaryOperator {
    fn from(pair: Pair<Rule>) -> Self {
        trace!("[unary_operator] {pair:?}");
        match pair.as_str() {
            "not" => UnaryOperator::Not,
            op => UnaryOperator::from_str(op)
                .unwrap_or_else(|_| unreachable!("Invalid operator {op}")),
        }
    }
}

impl Environment<'_> {
    pub fn eval_unary_operator(
        &self,
        ctx: &Context,
        operator: UnaryOperator,
        node: Node,
    ) -> Result<Value> {
        let node = self.eval_expr(ctx, node)?;
        let result = match operator {
            UnaryOperator::Not => match node {
                Bool(b) => Bool(!b),
                _ => bail!("Invalid operand for operator !"),
            },
            UnaryOperator::Positive => match node {
                Value::Number(n) => Value::Number(n),
                Value::Float(f) => Value::Float(f),
                _ => bail!("Invalid operand for operator +"),
            },
            UnaryOperator::Negative => match node {
                Value::Number(n) => Value::Number(-n),
                Value::Float(f) => Value::Float(-f),
                _ => bail!("Invalid operand for operator -"),
            },
        };

        Ok(result)
    }
}
