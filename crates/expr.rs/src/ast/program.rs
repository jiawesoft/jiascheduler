use crate::ast::node::Node;
use crate::Rule;
use pest::iterators::{Pair, Pairs};

/// A parsed expr program that can be run
#[derive(Debug, Clone, Default)]
pub struct Program {
    pub(crate) lines: Vec<(String, Node)>,
    pub(crate) expr: Node,
}

impl<'i> From<Pairs<'i, Rule>> for Program {
    fn from(pairs: Pairs<'i, Rule>) -> Self {
        let mut program = Program::default();
        for pair in pairs {
            if let Rule::EOI = pair.as_rule() {
                continue;
            }
            let p = Program::from(pair);
            program.lines.extend(p.lines);
            program.expr = p.expr;
        }
        program
    }
}

impl From<Pair<'_, Rule>> for Program {
    fn from(pair: Pair<'_, Rule>) -> Self {
        let mut lines = Vec::new();
        let mut expr = None;
        match pair.as_rule() {
            Rule::program => return pair.into_inner().into(),
            Rule::stmt => {
                let mut inner = pair.into_inner();
                let line = inner.next().unwrap().as_str().to_string();
                let node = Node::from(inner);
                lines.push((line, node));
            }
            Rule::expr => {
                expr = Some(Node::from(pair.into_inner()));
            }
            // means it's a predicate
            Rule::ident => {
                expr = Some(Node::Ident(pair.as_str().to_string()));
            }
            Rule::EOI => {}
            rule => unreachable!("Unexpected rule: {rule:?}"),
        }

        Program {
            lines,
            expr: expr.unwrap_or(Node::Value(crate::Value::Nil)),
        }
    }
}
