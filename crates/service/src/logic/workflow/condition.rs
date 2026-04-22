use crate::logic::workflow::condition;
use crate::logic::workflow::types::WorkflowNode;
use crate::{entity::prelude::*, state::AppContext};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use tracing::info;

use entity::workflow_process_node_task;
use expr::{Context, Environment};

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum ConditionValType {
    #[serde(rename = "user_variables")]
    UserVariables,
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "exit_code")]
    ExitCode,
    #[serde(rename = "output")]
    Output,
}

impl Display for ConditionValType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConditionValType::UserVariables => write!(f, "user_variables"),
            ConditionValType::Custom => write!(f, "custom"),
            ConditionValType::ExitCode => write!(f, "exit_code"),
            ConditionValType::Output => write!(f, "output"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ConditionVal {
    pub val_type: ConditionValType,
    pub val: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Condition {
    pub expr: String,
    pub logical_op: String,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub left_val: ConditionVal,
    pub op: String,
    pub right_val: ConditionVal,
}

impl Condition {
    pub fn get_wrap_val(&self, op: &str, var: &str, is_number: bool) -> String {
        match op {
            ">" | "<" | ">=" | "<=" => format!("float({})", var).to_string(),
            "==" | "!=" if is_number => format!("float({})", var).to_string(),
            _ => var.to_string(),
        }
    }

    pub async fn eval(&self, app_ctx: &AppContext, node: &WorkflowNode) -> Result<bool> {
        let mut global_ctx = Context::default();
        let mut outer_ctx = Context::default();
        let env = Environment::new();

        if let Some(vars) = node
            .process_args
            .as_ref()
            .and_then(|v| v.user_variables.as_ref())
        {
            vars.iter().for_each(|v| {
                global_ctx.insert(v.name.clone(), v.val.clone());
            });
        }

        for rule in self.rules.iter() {
            let mut ctx = global_ctx.clone();
            let d = match rule.left_val.val_type {
                condition::ConditionValType::UserVariables => match rule.right_val.val_type {
                    condition::ConditionValType::UserVariables => {
                        if rule.op == "contains" {
                            format!("indexOf({}, {}) > 0", rule.left_val.val, rule.right_val.val)
                        } else {
                            format!(
                                "{} {} {}",
                                self.get_wrap_val(&rule.op, &rule.left_val.val, false),
                                rule.op,
                                self.get_wrap_val(&rule.op, &rule.right_val.val, false)
                            )
                        }
                    }
                    condition::ConditionValType::Custom => {
                        ctx.insert("right_val", rule.right_val.val.clone());
                        if rule.op == "contains" {
                            format!("indexOf({}, right_val) > 0", rule.left_val.val)
                        } else {
                            format!(
                                "{} {} {}",
                                self.get_wrap_val(&rule.op, &rule.left_val.val, false),
                                rule.op,
                                self.get_wrap_val(&rule.op, "right_val", false)
                            )
                        }
                    }
                    condition::ConditionValType::ExitCode => {
                        let data = WorkflowProcessNodeTask::find()
                            .filter(
                                workflow_process_node_task::Column::ProcessId.eq(&node.process_id),
                            )
                            .filter(
                                workflow_process_node_task::Column::NodeId.eq(&rule.right_val.val),
                            )
                            .all(&app_ctx.db)
                            .await?;
                        ctx.insert("right_val", expr::to_value(&data)?);
                        if rule.op == "contains" {
                            format!(
                                "all(right_val,{{indexOf({}, string(#.exit_code)) > 0}})",
                                &rule.left_val.val,
                            )
                        } else {
                            format!(
                                "all(right_val,{{ {} {} {}}})",
                                self.get_wrap_val(&rule.op, &rule.left_val.val, true),
                                rule.op,
                                self.get_wrap_val(&rule.op, "#.exit_code", true),
                            )
                        }
                    }
                    condition::ConditionValType::Output => {
                        let data = WorkflowProcessNodeTask::find()
                            .filter(
                                workflow_process_node_task::Column::ProcessId.eq(&node.process_id),
                            )
                            .filter(
                                workflow_process_node_task::Column::NodeId.eq(&rule.right_val.val),
                            )
                            .all(&app_ctx.db)
                            .await?;
                        ctx.insert("right_val", expr::to_value(&data)?);

                        if rule.op == "contains" {
                            format!(
                                "all(right_val,{{indexOf({}, #.output,) > 0}})",
                                &rule.left_val.val,
                            )
                        } else {
                            format!(
                                "all(right_val,{{{} {} {}}})",
                                self.get_wrap_val(&rule.op, &rule.left_val.val, false),
                                rule.op,
                                self.get_wrap_val(&rule.op, "#.output", false),
                            )
                        }
                    }
                },
                condition::ConditionValType::Custom => {
                    ctx.insert("left_val", rule.left_val.val.clone());
                    match rule.right_val.val_type {
                        condition::ConditionValType::UserVariables => {
                            if rule.op == "contains" {
                                format!("indexOf(left_val, {}) > 0", rule.right_val.val)
                            } else {
                                format!(
                                    "{} {} {}",
                                    self.get_wrap_val(&rule.op, "left_val", false),
                                    rule.op,
                                    self.get_wrap_val(&rule.op, &rule.right_val.val, false),
                                )
                            }
                        }
                        condition::ConditionValType::Custom => {
                            ctx.insert("right_val", rule.right_val.val.clone());
                            if rule.op == "contains" {
                                format!("indexOf(left_val, right_val) > 0")
                            } else {
                                format!(
                                    "{} {} {}",
                                    self.get_wrap_val(&rule.op, "left_val", false),
                                    rule.op,
                                    self.get_wrap_val(&rule.op, "right_val", false)
                                )
                            }
                        }
                        condition::ConditionValType::ExitCode => {
                            let data = WorkflowProcessNodeTask::find()
                                .filter(
                                    workflow_process_node_task::Column::ProcessId
                                        .eq(&node.process_id),
                                )
                                .filter(
                                    workflow_process_node_task::Column::NodeId
                                        .eq(&rule.right_val.val),
                                )
                                .all(&app_ctx.db)
                                .await?;

                            ctx.insert("right_val", expr::to_value(&data)?);
                            if rule.op == "contains" {
                                format!(
                                    "all(right_val,{{indexOf(left_val, string(.exit_code)) > 0}})",
                                )
                            } else {
                                format!(
                                    "all(right_val,{{float(left_val) {} float(.exit_code)}})",
                                    rule.op
                                )
                            }
                        }
                        condition::ConditionValType::Output => {
                            let data = WorkflowProcessNodeTask::find()
                                .filter(
                                    workflow_process_node_task::Column::ProcessId
                                        .eq(&node.process_id),
                                )
                                .filter(
                                    workflow_process_node_task::Column::NodeId
                                        .eq(&rule.right_val.val),
                                )
                                .all(&app_ctx.db)
                                .await?;

                            ctx.insert("right_val", expr::to_value(&data)?);
                            if rule.op == "contains" {
                                format!("all(right_val,{{indexOf(left_val, .output) > 0}})",)
                            } else {
                                format!(
                                    "all(right_val,{{{} {} {}}})",
                                    self.get_wrap_val(&rule.op, "left_val", false),
                                    rule.op,
                                    self.get_wrap_val(&rule.op, ".output", false),
                                )
                            }
                        }
                    }
                }
                condition::ConditionValType::ExitCode => {
                    let data = WorkflowProcessNodeTask::find()
                        .filter(workflow_process_node_task::Column::ProcessId.eq(&node.process_id))
                        .filter(workflow_process_node_task::Column::NodeId.eq(&rule.left_val.val))
                        .all(&app_ctx.db)
                        .await?;

                    ctx.insert("left_val", expr::to_value(&data)?);
                    match rule.right_val.val_type {
                        condition::ConditionValType::UserVariables => {
                            if rule.op == "contains" {
                                format!(
                                    "all(left_val,{{indexOf(string(#.exit_code), {}) > 0}})",
                                    &rule.right_val.val
                                )
                            } else {
                                format!(
                                    "all(left_val,{{float(#.exit_code) {} {}}})",
                                    rule.op,
                                    self.get_wrap_val(&rule.op, &rule.right_val.val, true)
                                )
                            }
                        }
                        condition::ConditionValType::Custom => {
                            ctx.insert("right_val", &rule.right_val.val);
                            if rule.op == "contains" {
                                format!(
                                    "all(left_val,{{indexOf(string(#.exit_code), right_val) > 0}})",
                                )
                            } else {
                                format!(
                                    "all(left_val,{{float(#.exit_code) {} {}}})",
                                    rule.op,
                                    self.get_wrap_val(&rule.op, "right_val", true)
                                )
                            }
                        }
                        condition::ConditionValType::ExitCode => {
                            anyhow::bail!("not support exit code compare to exit code");
                        }
                        condition::ConditionValType::Output => {
                            anyhow::bail!("not support exit code compare to output");
                        }
                    }
                }
                condition::ConditionValType::Output => {
                    let data = WorkflowProcessNodeTask::find()
                        .filter(workflow_process_node_task::Column::ProcessId.eq(&node.process_id))
                        .filter(workflow_process_node_task::Column::NodeId.eq(&rule.left_val.val))
                        .all(&app_ctx.db)
                        .await?;

                    ctx.insert("left_val", expr::to_value(&data)?);
                    match rule.right_val.val_type {
                        condition::ConditionValType::UserVariables => {
                            if rule.op == "contains" {
                                format!(
                                    "all(left_val,{{indexOf(#.output, {}) > 0}})",
                                    rule.right_val.val
                                )
                            } else {
                                format!(
                                    "all(left_val,{{{} {} {}}})",
                                    self.get_wrap_val(&rule.op, "#.output", false),
                                    rule.op,
                                    self.get_wrap_val(&rule.op, &rule.right_val.val, false)
                                )
                            }
                        }
                        condition::ConditionValType::Custom => {
                            ctx.insert("right_val", &rule.right_val.val);
                            if rule.op == "contains" {
                                format!("all(left_val,{{indexOf(#.output, right_val) > 0}})",)
                            } else {
                                format!(
                                    "all(left_val,{{{} {} {}}})",
                                    self.get_wrap_val(&rule.op, "#.output", false),
                                    rule.op,
                                    self.get_wrap_val(&rule.op, "right_val", false),
                                )
                            }
                        }
                        condition::ConditionValType::ExitCode => {
                            anyhow::bail!("not support output compare to exit code");
                        }
                        condition::ConditionValType::Output => {
                            anyhow::bail!("not support output compare to output");
                        }
                    }
                }
            };
            let val = env
                .eval(d.as_str(), &ctx)?
                .as_bool()
                .ok_or(anyhow!("invalid express compare result"))?;
            info!("{}:{}, expr:{}", rule.name, val, d.as_str(),);

            outer_ctx.insert(rule.name.clone(), val);
        }

        expr::eval(&self.expr, &outer_ctx)?
            .as_bool()
            .ok_or(anyhow!("invalid express compare result"))
    }
}
