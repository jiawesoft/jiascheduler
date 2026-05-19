use core::matches;
use std::pin::Pin;
use std::str::FromStr;

use crate::logic::executor::ExecutorLogic;
use crate::logic::job::JobLogic;
use crate::logic::job::types::{DispatchData, DispatchResult, DispatchTarget};
use crate::logic::types::{ResourceType, UserInfo};
use crate::logic::workflow::types::{
    self, CustomJob, NodeStatus, NodeType, ProcessStatus, StandardJob, Task, TaskType,
    UserVariables, WorkflowNode, WorkflowNodeActualArgs, WorkflowProcessArgs,
};
use crate::{
    entity::{prelude::*, team_member},
    state::AppContext,
};
use anyhow::{Result, anyhow};
use automate::bridge::msg::UpdateJobParams;
use automate::scheduler::types::{RunStatus, UploadFile};
use chrono::{Local, Utc};
use croner::Cron;
use croner::parser::CronParser;
use entity::{
    executor, instance, job, tag_resource, team, workflow, workflow_process, workflow_process_edge,
    workflow_process_node, workflow_process_node_task, workflow_timer, workflow_version,
};
use local_ip_address::local_ip;
use redis::streams::{StreamMaxlen, StreamReadOptions, StreamReadReply};
use redis::{AsyncCommands, from_redis_value};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, JoinType, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, QueryTrait,
};
use sea_query::{Expr, Query};
use serde_json::json;
use tokio::fs;
use tracing::{error, info, warn};
use utils::file_name;

use super::types::{EdgeConfig, NodeConfig};

pub struct WorkflowLogic<'a> {
    pub(super) ctx: &'a AppContext,
}

impl<'a> WorkflowLogic<'a> {
    pub const WORKFLOW_TOPIC: &'static str = "jiascheduler:workflow";
    pub const WORKFLOW_TIMER_TOPIC: &'static str = "jiascheduler:workflow:timer";

    pub const CONSUMER_GROUP: &'static str = "jiascheduler-group";

    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn get_workflow_list(
        &self,
        _user_info: &UserInfo,
        created_user: Option<&str>,
        default_id: Option<u64>,
        team_id: Option<u64>,
        tag_ids: Option<Vec<u64>>,
        name: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::WorkflowModel>, u64)> {
        let mut select = Workflow::find()
            .column_as(team::Column::Name, "team_name")
            .apply_if(team_id, |q, v| q.filter(workflow::Column::TeamId.eq(v)))
            .apply_if(name, |q, v| q.filter(workflow::Column::Name.contains(v)))
            .apply_if(created_user, |q, v| {
                q.filter(workflow::Column::CreatedUser.eq(v))
            })
            .filter(workflow::Column::IsDeleted.eq(false))
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Workflow)
                    .from(team::Column::Id)
                    .to(workflow::Column::TeamId)
                    .into(),
            );

        if let Some(v) = tag_ids {
            select = select.filter(
                Condition::any().add(
                    workflow::Column::Id.in_subquery(
                        Query::select()
                            .column(tag_resource::Column::ResourceId)
                            .and_where(
                                tag_resource::Column::ResourceType
                                    .eq(ResourceType::Workflow.to_string())
                                    .and(tag_resource::Column::TagId.is_in(v)),
                            )
                            .from(TagResource)
                            .to_owned(),
                    ),
                ),
            );
        }

        let total = select.clone().count(&self.ctx.db).await?;
        let ret = select
            .apply_if(default_id, |query, v| {
                query.order_by_desc(Expr::expr(workflow::Column::Id.eq(v)))
            })
            .order_by_desc(workflow::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((ret, total))
    }

    pub async fn get_workflow_process_list(
        &self,
        _user_info: &UserInfo,
        created_user: Option<&str>,
        default_id: Option<u64>,
        team_id: Option<u64>,
        tag_ids: Option<Vec<u64>>,
        name: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<types::WorkflowProcessModel>, u64)> {
        let mut select = WorkflowProcess::find()
            .column_as(workflow::Column::TeamId, "team_id")
            .column_as(workflow::Column::Name, "workflow_name")
            .column_as(team::Column::Name, "team_name")
            .column_as(workflow_version::Column::Nodes, "workflow_nodes")
            .column_as(workflow_version::Column::Version, "version")
            .column_as(workflow_timer::Column::Name, "timer_name")
            .apply_if(team_id, |q, v| q.filter(workflow::Column::TeamId.eq(v)))
            .apply_if(name, |q, v| {
                q.filter(workflow_process::Column::ProcessName.contains(v))
            })
            .apply_if(created_user, |q, v| {
                q.filter(workflow_process::Column::CreatedUser.eq(v))
            })
            .apply_if(default_id, |q, v| {
                q.filter(workflow_process::Column::Id.eq(v))
            })
            .join_rev(
                JoinType::LeftJoin,
                Workflow::belongs_to(WorkflowProcess)
                    .from(workflow::Column::Id)
                    .to(workflow_process::Column::WorkflowId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                WorkflowTimer::belongs_to(WorkflowProcess)
                    .from(workflow_timer::Column::Id)
                    .to(workflow_process::Column::TimerId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Workflow)
                    .from(team::Column::Id)
                    .to(workflow::Column::TeamId)
                    .into(),
            )
            .join_rev(
                JoinType::LeftJoin,
                WorkflowVersion::belongs_to(WorkflowProcess)
                    .from(workflow_version::Column::Id)
                    .to(workflow_process::Column::VersionId)
                    .into(),
            )
            .filter(workflow_process::Column::IsDeleted.eq(false));

        if let Some(v) = tag_ids {
            select = select.filter(
                Condition::any().add(
                    workflow::Column::Id.in_subquery(
                        Query::select()
                            .column(tag_resource::Column::ResourceId)
                            .and_where(
                                tag_resource::Column::ResourceType
                                    .eq(ResourceType::Workflow.to_string())
                                    .and(tag_resource::Column::TagId.is_in(v)),
                            )
                            .from(TagResource)
                            .to_owned(),
                    ),
                ),
            );
        }

        let total = select.clone().count(&self.ctx.db).await?;
        let ret = select
            .order_by_desc(workflow_process::Column::Id)
            .into_model()
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page - 1)
            .await?;
        Ok((ret, total))
    }

    pub async fn get_workflow_version_list(
        &self,
        _user_info: &UserInfo,
        version: Option<String>,
        created_user: Option<String>,
        workflow_id: u64,
        id: Option<u64>,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<workflow_version::Model>, u64)> {
        let select = WorkflowVersion::find()
            .filter(workflow_version::Column::WorkflowId.eq(workflow_id))
            .apply_if(created_user, |q, v| {
                q.filter(workflow_version::Column::CreatedUser.eq(v))
            })
            .apply_if(version, |q, v| {
                q.filter(workflow_version::Column::Version.contains(v))
            })
            .apply_if(id, |q, v| q.filter(workflow_version::Column::Id.eq(v)))
            .filter(workflow_version::Column::IsDeleted.eq(false));

        let total = select.clone().count(&self.ctx.db).await?;
        let ret = select
            .order_by_desc(workflow_version::Column::Id)
            .paginate(&self.ctx.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((ret, total))
    }

    pub async fn can_write_timer(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        timer_id: Option<u64>,
    ) -> Result<bool> {
        let Some(timer_id) = timer_id else {
            return Ok(true);
        };
        let Some(timer) = WorkflowTimer::find()
            .filter(workflow_timer::Column::Id.eq(timer_id))
            .one(&self.ctx.db)
            .await?
        else {
            anyhow::bail!("cannot found timer {timer_id}");
        };

        self.can_write_workflow(user_info, team_id, Some(timer.workflow_id))
            .await
    }

    pub async fn can_write_workflow(
        &self,
        user_info: &UserInfo,
        team_id: Option<u64>,
        workflow_id: Option<u64>,
    ) -> Result<bool> {
        let is_allowed = self.ctx.can_manage_job(&user_info.user_id).await?;
        if is_allowed {
            return Ok(true);
        }

        let is_team_user = if team_id.is_some() {
            TeamMember::find()
                .apply_if(team_id, |q, v| q.filter(team_member::Column::TeamId.eq(v)))
                .filter(team_member::Column::UserId.eq(&user_info.user_id))
                .one(&self.ctx.db)
                .await?
                .map(|_| true)
        } else {
            None
        };

        let Some(workflow_id) = workflow_id else {
            return Ok(is_team_user.is_some() || team_id == Some(0) || team_id.is_none());
        };

        let Some(workflow_record) = Workflow::find()
            .filter(workflow::Column::Id.eq(workflow_id))
            .one(&self.ctx.db)
            .await?
        else {
            return Ok(false);
        };

        if workflow_record.created_user == user_info.username {
            return Ok(true);
        }

        if is_team_user.is_some() {
            return Ok(Some(workflow_record.team_id) == team_id);
        }
        return Ok(TeamMember::find()
            .apply_if(Some(workflow_record.team_id), |q, v| {
                q.filter(team_member::Column::TeamId.eq(v))
            })
            .filter(team_member::Column::UserId.eq(&user_info.user_id))
            .one(&self.ctx.db)
            .await?
            .map(|_| true)
            == Some(true));
    }

    pub fn check_nodes(
        nodes: Option<Vec<NodeConfig>>,
        edges: Option<Vec<EdgeConfig>>,
    ) -> Result<(Option<Vec<NodeConfig>>, Option<Vec<EdgeConfig>>)> {
        let mut has_start_node = false;
        let mut has_end_node = false;

        let Some(nodes) = nodes else {
            return Ok((None, edges));
        };

        for node in &nodes {
            if node.task_type == TaskType::Standard
                && matches!(node.task, Task::Standard(StandardJob{ref eid,..}) if eid == "")
            {
                anyhow::bail!("no job is assigned to the workflow node {}", node.name)
            }
            if node.task_type == TaskType::Custom
                && matches!(node.task, Task::Custom(CustomJob{ executor_id, ref code, ..}) if executor_id == 0 || code == "")
            {
                anyhow::bail!(
                    "no custom task is assigned to the workflow node {}",
                    node.name
                )
            }

            if node.name == "" {
                anyhow::bail!("node name cannot be empty")
            }
            if node.id == "" {
                anyhow::bail!("node id cannot be empty")
            }

            if node.node_type == NodeType::StartEvent {
                has_start_node = true
            }

            if node.node_type == NodeType::EndEvent {
                has_end_node = true
            }
            let node_id = node.id.as_str();
            let is_connected = edges.as_ref().is_some_and(|v| {
                v.iter()
                    .any(|v| v.source_node_id == node_id || v.target_node_id == node_id)
            });

            if !is_connected {
                anyhow::bail!(
                    "{} is an isolated node. Please ensure the workflow is a valid directed acyclic graph (DAG)",
                    node.name
                )
            }
        }

        if !has_start_node {
            anyhow::bail!("workflow should has a start node")
        }
        if !has_end_node {
            anyhow::bail!("workflow should has a end node")
        }

        Ok((Some(nodes), edges))
    }

    pub async fn save_workflow(
        &self,
        id: Option<u64>,
        user_info: &UserInfo,
        name: String,
        info: Option<String>,
        team_id: Option<u64>,
        nodes: Option<Vec<NodeConfig>>,
        edges: Option<Vec<EdgeConfig>>,
        user_variables: Option<Vec<UserVariables>>,
    ) -> Result<u64> {
        let (nodes, edges) = Self::check_nodes(nodes, edges)?;

        let nodes = match nodes {
            Some(v) => Set(Some(serde_json::to_value(v)?)),
            None => NotSet,
        };

        let edges = match edges {
            Some(v) => Set(Some(serde_json::to_value(v)?)),
            None => NotSet,
        };
        let user_variables = match user_variables {
            Some(v) => Set(Some(serde_json::to_value(v)?)),
            None => NotSet,
        };

        let active_model = workflow::ActiveModel {
            id: id.map_or(NotSet, |v| Set(v)),
            name: Set(name),
            info: info.map_or(NotSet, |v| Set(v)),
            team_id: team_id.map_or(NotSet, |v| Set(v)),
            created_user: Set(user_info.username.clone()),
            updated_user: Set(user_info.username.clone()),
            nodes,
            edges,
            user_variables,
            ..Default::default()
        };

        if let Some(id) = id {
            let affected = Workflow::update_many()
                .set(active_model)
                .filter(workflow::Column::Id.eq(id))
                .filter(workflow::Column::IsDeleted.eq(false))
                .exec(&self.ctx.db)
                .await?
                .rows_affected;
            return Ok(affected);
        }

        let active_model = active_model.save(&self.ctx.db).await?;
        Ok(active_model.id.as_ref().to_owned())
    }

    pub async fn release_version(
        &self,
        workflow_id: u64,
        user_info: &UserInfo,
        version: String,
        version_info: Option<String>,
        nodes: Option<Vec<NodeConfig>>,
        edges: Option<Vec<EdgeConfig>>,
        user_variables: Option<Vec<UserVariables>>,
        team_id: Option<u64>,
    ) -> Result<u64> {
        let (nodes, edges) = Self::check_nodes(nodes, edges)?;

        let user_variables_val = Set(user_variables
            .map(|v| serde_json::to_value(v))
            .transpose()?);

        workflow::ActiveModel {
            id: Set(workflow_id),
            team_id: team_id.map_or(NotSet, |v| Set(v)),
            nodes: Set(nodes.clone().map(|v| serde_json::to_value(v)).transpose()?),
            edges: Set(edges.clone().map(|v| serde_json::to_value(v)).transpose()?),
            user_variables: user_variables_val.clone(),
            created_user: Set(user_info.username.clone()),
            updated_user: Set(user_info.username.clone()),
            ..Default::default()
        }
        .save(&self.ctx.db)
        .await?;

        let ret = workflow_version::ActiveModel {
            workflow_id: Set(workflow_id),
            team_id: team_id.map_or(NotSet, |v| Set(v)),
            version: Set(version),
            version_info: version_info.map_or(NotSet, |v| Set(v)),
            nodes: Set(nodes.map(|v| serde_json::to_value(v)).transpose()?),
            edges: Set(edges.map(|v| serde_json::to_value(v)).transpose()?),
            created_user: Set(user_info.username.clone()),
            user_variables: user_variables_val,
            ..Default::default()
        }
        .save(&self.ctx.db)
        .await?;

        Ok(ret.id.as_ref().to_owned())
    }

    pub async fn get_workflow_detail(
        &self,
        workflow_id: u64,
        version_id: Option<u64>,
    ) -> Result<types::WorkflowVersionDetailModel> {
        let workflow_record: types::WorkflowModel = Workflow::find()
            .filter(workflow::Column::Id.eq(workflow_id))
            .join_rev(
                JoinType::LeftJoin,
                Team::belongs_to(Workflow)
                    .from(team::Column::Id)
                    .to(workflow::Column::TeamId)
                    .into(),
            )
            .into_model()
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("not found workflow {}", workflow_id))?;

        let mut ret = types::WorkflowVersionDetailModel {
            workflow_id: workflow_id,
            workflow_name: workflow_record.name,
            workflow_info: workflow_record.info,
            nodes: workflow_record.nodes,
            edges: workflow_record.edges,
            user_variables: workflow_record.user_variables,
            team_id: workflow_record.team_id,
            created_user: workflow_record.created_user,
            updated_user: workflow_record.updated_user,
            created_time: workflow_record.created_time,
            updated_time: workflow_record.updated_time,
            ..Default::default()
        };

        let Some(version_id) = version_id else {
            return Ok(ret);
        };

        let version_record = WorkflowVersion::find()
            .filter(workflow_version::Column::Id.eq(version_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("not found workflow version {}", version_id))?;

        ret.version = Some(version_record.version);
        ret.version_id = Some(version_record.id);
        ret.version_info = Some(version_record.version_info);
        ret.nodes = version_record.nodes;
        ret.edges = version_record.edges;
        ret.user_variables = version_record.user_variables;

        Ok(ret)
    }

    pub async fn get_process_detail(
        &self,
        process_id: String,
    ) -> Result<types::WorkflowProcessDetail> {
        let process_record = WorkflowProcess::find()
            .filter(workflow_process::Column::ProcessId.eq(&process_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("not found"))?;

        let workflow_version_record = WorkflowVersion::find()
            .filter(workflow_version::Column::WorkflowId.eq(process_record.workflow_id))
            .filter(workflow_version::Column::Id.eq(process_record.version_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("not found"))?;

        let completed_node = WorkflowProcessNode::find()
            .filter(workflow_process_node::Column::ProcessId.eq(&process_id))
            .order_by_desc(workflow_process_node::Column::Depth)
            .order_by_desc(workflow_process_node::Column::Id)
            .all(&self.ctx.db)
            .await?;
        let completed_node_task = WorkflowProcessNodeTask::find()
            .filter(workflow_process_node_task::Column::ProcessId.eq(&process_id))
            .order_by_desc(workflow_process_node_task::Column::Id)
            .all(&self.ctx.db)
            .await?;
        let completed_edge = WorkflowProcessEdge::find()
            .filter(workflow_process_edge::Column::ProcessId.eq(&process_id))
            .all(&self.ctx.db)
            .await?;

        let completed_nodes = completed_node
            .into_iter()
            .map(|v| {
                let node_id = v.node_id.clone();
                let run_id = v.run_id.clone();
                let data = types::WorkflowProcessCompletedNode {
                    base: v,
                    tasks: completed_node_task
                        .iter()
                        .filter_map(|task| {
                            if task.node_id == node_id && task.run_id == run_id {
                                Some(task.clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                };
                data
            })
            .collect();
        let completed_edges = completed_edge
            .into_iter()
            .map(|v| types::WorkflowProcessCompletedEdge { base: v })
            .collect();

        let detail = types::WorkflowProcessDetail {
            process_id,
            process_name: process_record.process_name,
            created_user: process_record.created_user,
            current_run_id: process_record.current_run_id,
            current_node_id: process_record.current_node_id,
            current_node_status: process_record.current_node_status,
            process_status: process_record.process_status,
            origin_nodes: workflow_version_record.nodes,
            origin_edges: workflow_version_record.edges,
            process_args: process_record.process_args,
            completed_nodes,
            completed_edges,
        };

        Ok(detail)
    }

    async fn send_msg<'b>(&self, items: &'b [(&'b str, WorkflowNode)]) -> Result<String> {
        let mut conn = self.ctx.redis().get_multiplexed_async_connection().await?;
        let v: String = conn.xadd(Self::WORKFLOW_TOPIC, "*", items).await?;
        Ok(v)
    }

    pub async fn flow_next(&self, node: WorkflowNode) -> Result<String> {
        let val = self.send_msg(&[("workflow", node)]).await?;
        Ok(val)
    }

    pub async fn handle_start_event(&self, node: &WorkflowNode) -> Result<()> {
        let next_point = node.get_next_nodes()?;

        WorkflowProcessNode::update_many()
            .set(workflow_process_node::ActiveModel {
                node_status: Set(NodeStatus::End.to_string()),
                ..Default::default()
            })
            .filter(workflow_process_node::Column::ProcessId.eq(&node.process_id))
            .filter(workflow_process_node::Column::NodeId.eq(&node.current_node.id))
            .filter(workflow_process_node::Column::RunId.eq(&node.run_id))
            .exec(&self.ctx.db)
            .await?;

        for point in next_point {
            let mut next_node = node.clone();
            next_node.reached_edge = Some(point.0.clone());
            next_node.current_node = point.1.clone();
            let _ = self.flow_next(next_node).await?;
        }

        Ok(())
    }

    pub async fn parse_actual_args(
        &'_ self,
        node: &'a mut WorkflowNode,
        code: String,
    ) -> Result<&'a WorkflowNodeActualArgs> {
        let process_node_args = node.process_args.as_ref().map_or(None, |v| {
            v.nodes
                .iter()
                .find_map(|v| v.iter().find(|&v| v.node_id == node.current_node.id))
        });

        let mut formal_args = match node.current_node.task {
            Task::Standard(ref standard_job) => standard_job.formal_args.clone(),
            Task::Custom(ref custom_job) => custom_job.formal_args.clone(),
            Task::None => vec![],
        };

        let actual_args = process_node_args.map_or(None, |v| v.args.clone());
        let mut args = json!({});

        if let Some(actual_args) = actual_args {
            formal_args.iter_mut().for_each(|v| {
                actual_args.iter().for_each(|item| {
                    if item.name == v.name {
                        v.val = item.val.clone();
                        v.val_type = item.val_type.clone();
                    }
                });
            });
        }

        for arg in formal_args.clone() {
            if arg.val_type.eq("dynamic") {
                let records = WorkflowProcessNodeTask::find()
                    .filter(workflow_process_node_task::Column::ProcessId.eq(&node.process_id))
                    .filter(workflow_process_node_task::Column::NodeId.eq(arg.val))
                    .all(&self.ctx.db)
                    .await?;

                args[arg.name] = serde_json::to_value(records)?
            } else {
                args[arg.name] = serde_json::to_value(arg.val)?;
            }
        }

        let code = JobLogic::get_job_code(code, Some(args.clone()))?;

        let mut target = match node.current_node.task.clone() {
            Task::Standard(standard_job) => standard_job.target,
            Task::Custom(custom_job) => custom_job.target,
            Task::None => None,
        };

        if let Some(current_args) = process_node_args
            && current_args.target.as_ref().is_some_and(|v| v.len() > 0)
        {
            target = current_args.target.clone();
        }

        if !target.clone().is_some_and(|v| v.len() > 0) {
            target = node
                .process_args
                .as_ref()
                .map_or(None, |v| v.default_target.clone());
        }
        let Some(target) = target.clone() else {
            anyhow::bail!("execution target is empty");
        };

        node.actual_args = Some(WorkflowNodeActualArgs {
            formal: formal_args,
            args: Some(args),
            code: code,
            target,
        });

        Ok(node.actual_args.as_ref().unwrap())
    }

    pub async fn dispatch_custom_job(
        &self,
        node: &mut WorkflowNode,
        custom_job: &CustomJob,
    ) -> Result<()> {
        let executor_record = Executor::find()
            .filter(executor::Column::Id.eq(custom_job.executor_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!(
                "cannot found executor {}",
                custom_job.executor_id.clone()
            ))?;

        let upload_file: Option<UploadFile> = if let Some(uploadfile) =
            custom_job.upload_file.clone()
            && uploadfile != ""
        {
            let data = fs::read(uploadfile.clone()).await?;
            Some(UploadFile {
                filename: file_name!(uploadfile),
                data: Some(data),
            })
        } else {
            None
        };

        let (cmd_name, cmd_args) = ExecutorLogic::get_cmd_args(&executor_record);
        let eid = node.current_node.id.clone();
        let schedule_id = node.process_id.clone();

        let (process_id, node_id, run_id) = (
            node.process_id.clone(),
            node.current_node.id.clone(),
            node.run_id.clone(),
        );

        let actual_args = self
            .parse_actual_args(node, custom_job.code.clone())
            .await?;

        let endpoints = Instance::find()
            .filter(instance::Column::InstanceId.is_in(&actual_args.target))
            .all(&self.ctx.db)
            .await?;
        if endpoints.len() == 0 {
            anyhow::bail!("cannot found valid instance");
        }

        WorkflowProcessNode::update_many()
            .set(workflow_process_node::ActiveModel {
                node_args: Set(Some(serde_json::to_value(actual_args)?)),
                ..Default::default()
            })
            .filter(workflow_process_node::Column::ProcessId.eq(process_id))
            .filter(workflow_process_node::Column::NodeId.eq(node_id))
            .filter(workflow_process_node::Column::RunId.eq(run_id))
            .exec(&self.ctx.db)
            .await?;

        let dispatch_params = automate::DispatchJobParams {
            base_job: automate::BaseJob {
                eid,
                cmd_name,
                upload_file,
                code: actual_args.code.clone(),
                args: cmd_args,
                timeout: 60,
                max_retry: Some(1),
                max_parallel: Some(1),
                read_code_from_stdin: false,
                is_workflow: true,
                ..Default::default()
            },
            run_id: node.run_id.clone(),
            instance_id: None,
            fields: Some(json!({
                "workflow_node": serde_json::to_value(& *node)?
            })),
            restart_interval: None,
            created_user: node.created_user.clone(),
            schedule_id,
            timer_expr: None,
            is_sync: false,
            action: automate::JobAction::Exec,
        };

        let mut dispatch_data = DispatchData {
            target: Vec::new(),
            params: dispatch_params.clone(),
        };

        endpoints.into_iter().for_each(|v| {
            dispatch_data.target.push(DispatchTarget {
                ip: v.ip.clone(),
                mac_addr: v.mac_addr.clone(),
                namespace: v.namespace.clone(),
                instance_id: v.instance_id.clone(),
            });
        });

        let logic = automate::Logic::new(self.ctx.redis().clone());
        let http_client = self.ctx.http_client.clone();
        let secret = "".to_string();

        let batch_push_ret = utils::async_batch_do(dispatch_data.target.clone(), move |v| {
            let mut dispatch_params = dispatch_params.clone();
            let logic = logic.clone();
            let http_client = http_client.clone();
            let secret = secret.clone();
            dispatch_params.instance_id = Some(v.instance_id.clone());
            Box::pin(async move {
                let body = automate::DispatchJobRequest {
                    agent_ip: v.ip.clone(),
                    mac_addr: v.mac_addr.clone(),
                    dispatch_params: dispatch_params.clone(),
                };
                let pair = match logic.get_link_pair(v.ip.clone(), v.mac_addr.clone()).await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            instance_id: v.instance_id.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };
                let api_url = format!(
                    "http://{}/dispatch?secret={}",
                    pair.1.comet_addr,
                    secret.clone()
                );
                let response = match http_client.post(api_url).json(&body).send().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            instance_id: v.instance_id.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };

                let response = match response.error_for_status() {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            instance_id: v.instance_id.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };

                let ret = match response.json::<serde_json::Value>().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            instance_id: v.instance_id.clone(),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };

                let (has_err, err) = if ret["code"] != 20000 {
                    (true, Some(ret["msg"].to_string()))
                } else {
                    (false, None)
                };

                Ok(DispatchResult {
                    namespace: v.namespace.clone(),
                    bind_ip: v.ip.clone(),
                    response: ret.clone(),
                    instance_id: v.instance_id.clone(),
                    has_err,
                    err,
                })
            })
        })
        .await;

        let data = batch_push_ret
            .into_iter()
            .filter_map(|v| {
                v.map_or(None, |v| {
                    Some(workflow_process_node_task::ActiveModel {
                        process_id: Set(node.process_id.clone()),
                        node_id: Set(node.current_node.id.clone()),
                        run_id: Set(node.run_id.clone()),
                        task_status: Set("prepare".to_string()),
                        bind_ip: Set(v.bind_ip.clone()),
                        dispatch_result: Set(Some(serde_json::to_value(&v).unwrap())),
                        created_user: Set(node.created_user.clone()),
                        output: Set("".to_string()),
                        ..Default::default()
                    })
                })
            })
            .collect::<Vec<entity::workflow_process_node_task::ActiveModel>>();

        WorkflowProcessNodeTask::insert_many(data)
            .exec(&self.ctx.db)
            .await?;

        Ok(())
    }

    pub async fn dispatch_job(&self, node: &mut WorkflowNode, std_job: &StandardJob) -> Result<()> {
        let job_record = Job::find()
            .filter(job::Column::Eid.eq(std_job.eid.clone()))
            .filter(job::Column::IsDeleted.eq(false))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("cannot found job {}", std_job.eid))?;

        let executor_record = Executor::find()
            .filter(executor::Column::Id.eq(job_record.executor_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!(
                "cannot found executor {}",
                job_record.executor_id.clone()
            ))?;

        let mut upload_file: Option<UploadFile> = None;

        if job_record.upload_file != "" {
            let data = fs::read(job_record.upload_file.clone()).await?;
            upload_file = Some(UploadFile {
                filename: file_name!(job_record.upload_file.clone()),
                data: Some(data),
            });
        }

        let (cmd_name, cmd_args) = ExecutorLogic::get_cmd_args(&executor_record);
        let eid = node.current_node.id.clone();
        let schedule_id = node.process_id.clone();

        let (process_id, node_id, run_id) = (
            node.process_id.clone(),
            node.current_node.id.clone(),
            node.run_id.clone(),
        );

        let actual_args = self
            .parse_actual_args(node, job_record.code.clone())
            .await?;

        let endpoints = Instance::find()
            .filter(instance::Column::InstanceId.is_in(&actual_args.target))
            .all(&self.ctx.db)
            .await?;
        if endpoints.len() == 0 {
            anyhow::bail!("cannot found valid instance");
        }

        WorkflowProcessNode::update_many()
            .set(workflow_process_node::ActiveModel {
                node_args: Set(Some(serde_json::to_value(actual_args)?)),
                ..Default::default()
            })
            .filter(workflow_process_node::Column::ProcessId.eq(process_id))
            .filter(workflow_process_node::Column::NodeId.eq(node_id))
            .filter(workflow_process_node::Column::RunId.eq(run_id))
            .exec(&self.ctx.db)
            .await?;

        let dispatch_params = automate::DispatchJobParams {
            base_job: automate::BaseJob {
                eid,
                cmd_name,
                code: actual_args.code.clone(),
                args: cmd_args,
                upload_file: upload_file,
                work_dir: Some(job_record.work_dir.clone()).filter(|v| !v.is_empty()),
                work_user: Some(job_record.work_user.clone()).filter(|v| !v.is_empty()),
                timeout: job_record.timeout,
                max_retry: Some(job_record.max_retry),
                max_parallel: Some(job_record.max_parallel.into()),
                read_code_from_stdin: false,
                is_workflow: true,
                ..Default::default()
            },
            run_id: node.run_id.clone(),
            instance_id: None,
            fields: Some(json!({
                "workflow_node": serde_json::to_value(& *node)?
            })),
            restart_interval: None,
            created_user: node.created_user.clone(),
            schedule_id,
            timer_expr: None,
            is_sync: false,
            action: automate::JobAction::Exec,
        };

        let mut dispatch_data = DispatchData {
            target: Vec::new(),
            params: dispatch_params.clone(),
        };

        endpoints.into_iter().for_each(|v| {
            dispatch_data.target.push(DispatchTarget {
                ip: v.ip.clone(),
                mac_addr: v.mac_addr.clone(),
                namespace: v.namespace.clone(),
                instance_id: v.instance_id.clone(),
            });
        });

        let logic = automate::Logic::new(self.ctx.redis().clone());
        let http_client = self.ctx.http_client.clone();
        let secret = "".to_string();

        let batch_push_ret = utils::async_batch_do(dispatch_data.target.clone(), move |v| {
            let mut dispatch_params = dispatch_params.clone();
            let logic = logic.clone();
            let http_client = http_client.clone();
            let secret = secret.clone();
            dispatch_params.instance_id = Some(v.instance_id.clone());
            Box::pin(async move {
                let body = automate::DispatchJobRequest {
                    agent_ip: v.ip.clone(),
                    mac_addr: v.mac_addr.clone(),
                    dispatch_params: dispatch_params.clone(),
                };
                let pair = match logic.get_link_pair(v.ip.clone(), v.mac_addr.clone()).await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            instance_id: v.instance_id.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };
                let api_url = format!(
                    "http://{}/dispatch?secret={}",
                    pair.1.comet_addr,
                    secret.clone()
                );
                let response = match http_client.post(api_url).json(&body).send().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            instance_id: v.instance_id.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };

                let response = match response.error_for_status() {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            instance_id: v.instance_id.clone(),
                            response: json!(null),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };

                let ret = match response.json::<serde_json::Value>().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(DispatchResult {
                            namespace: v.namespace.clone(),
                            bind_ip: v.ip.clone(),
                            response: json!(null),
                            instance_id: v.instance_id.clone(),
                            has_err: true,
                            err: Some(e.to_string()),
                        });
                    }
                };

                let (has_err, err) = if ret["code"] != 20000 {
                    (true, Some(ret["msg"].to_string()))
                } else {
                    (false, None)
                };

                Ok(DispatchResult {
                    namespace: v.namespace.clone(),
                    bind_ip: v.ip.clone(),
                    response: ret.clone(),
                    instance_id: v.instance_id.clone(),
                    has_err,
                    err,
                })
            })
        })
        .await;

        let data = batch_push_ret
            .into_iter()
            .filter_map(|v| {
                v.map_or(None, |v| {
                    Some(workflow_process_node_task::ActiveModel {
                        process_id: Set(node.process_id.clone()),
                        node_id: Set(node.current_node.id.clone()),
                        run_id: Set(node.run_id.clone()),
                        task_status: Set("prepare".to_string()),
                        bind_ip: Set(v.bind_ip.clone()),
                        dispatch_result: Set(Some(serde_json::to_value(&v).unwrap())),
                        created_user: Set(node.created_user.clone()),
                        output: Set("".to_string()),
                        ..Default::default()
                    })
                })
            })
            .collect::<Vec<entity::workflow_process_node_task::ActiveModel>>();

        WorkflowProcessNodeTask::insert_many(data)
            .exec(&self.ctx.db)
            .await?;

        Ok(())
    }

    pub async fn handle_service_task(&self, node: &mut WorkflowNode) -> Result<()> {
        match node.current_node.task {
            Task::Standard(ref standard_job) => {
                self.dispatch_job(node, &standard_job.clone()).await?;
            }
            Task::Custom(ref custom_job) => {
                self.dispatch_custom_job(node, &custom_job.clone()).await?
            }
            Task::None => todo!(),
        }

        Ok(())
    }

    pub async fn is_continue(&self, node: &mut WorkflowNode) -> Result<bool> {
        if let Some(ref reached_edge) = node.reached_edge {
            WorkflowProcessEdge::insert(workflow_process_edge::ActiveModel {
                process_id: Set(node.process_id.to_string()),
                run_id: Set(node.run_id.to_string()),
                edge_id: Set(reached_edge.id.to_string()),
                props: Set(Some(serde_json::to_value(reached_edge)?)),
                source_node_id: Set(reached_edge.source_node_id.to_string()),
                target_node_id: Set(reached_edge.target_node_id.to_string()),
                created_user: Set(node.created_user.to_string()),
                ..Default::default()
            })
            .exec(&self.ctx.db)
            .await?;
        }

        let is_ready = if node.current_node.is_join_all {
            let prev_edges = node.get_prev_edges();

            let ready_edges = WorkflowProcessEdge::find()
                .filter(workflow_process_edge::Column::ProcessId.eq(node.process_id.clone()))
                .filter(
                    workflow_process_edge::Column::TargetNodeId.eq(node.current_node.id.clone()),
                )
                .filter(workflow_process_edge::Column::RunId.eq(node.run_id.clone()))
                .all(&self.ctx.db)
                .await?;

            prev_edges
                .iter()
                .all(|&v1| ready_edges.iter().any(|v2| v2.edge_id == v1.id))
        } else {
            true
        };

        if is_ready {
            let affected = WorkflowProcessNode::update_many()
                .set(workflow_process_node::ActiveModel {
                    updated_time: Set(Local::now()),
                    depth: Set(node.flow_depth),
                    ..Default::default()
                })
                .filter(workflow_process_node::Column::ProcessId.eq(&node.process_id))
                .filter(workflow_process_node::Column::RunId.eq(&node.run_id))
                .filter(workflow_process_node::Column::NodeId.eq(&node.current_node.id))
                .exec(&self.ctx.db)
                .await?
                .rows_affected;

            if affected > 0 {
                return Ok(false || node.current_node.node_type == NodeType::EndEvent);
            }

            WorkflowProcess::update_many()
                .set(workflow_process::ActiveModel {
                    current_node_id: Set(node.current_node.id.to_string()),
                    current_node_status: Set(NodeStatus::Running.to_string()),
                    current_run_id: Set(node.run_id.to_string()),
                    ..Default::default()
                })
                .filter(workflow_process::Column::ProcessId.eq(&node.process_id))
                .exec(&self.ctx.db)
                .await?;

            WorkflowProcessNode::insert(workflow_process_node::ActiveModel {
                process_id: Set(node.process_id.to_string()),
                run_id: Set(node.run_id.to_string()),
                node_id: Set(node.current_node.id.to_string()),
                node_status: Set(NodeStatus::Running.to_string()),
                depth: Set(node.flow_depth),
                created_user: Set(node.created_user.to_string()),
                ..Default::default()
            })
            .exec(&self.ctx.db)
            .await?;
        }

        Ok(is_ready)
    }

    pub async fn handle_exclusive_gateway(&self, node: &WorkflowNode) -> Result<()> {
        let next_edges = node.get_next_edges();

        WorkflowProcessNode::update_many()
            .set(workflow_process_node::ActiveModel {
                node_status: Set(NodeStatus::End.to_string()),
                ..Default::default()
            })
            .filter(workflow_process_node::Column::ProcessId.eq(&node.process_id))
            .filter(workflow_process_node::Column::NodeId.eq(&node.current_node.id))
            .filter(workflow_process_node::Column::RunId.eq(&node.run_id))
            .exec(&self.ctx.db)
            .await?;

        WorkflowProcess::update_many()
            .set(workflow_process::ActiveModel {
                current_node_status: Set(NodeStatus::End.to_string()),
                ..Default::default()
            })
            .filter(workflow_process::Column::ProcessId.eq(&node.process_id))
            .exec(&self.ctx.db)
            .await?;

        for edge in next_edges {
            let pass = if let Some(ref c) = edge.condition {
                c.eval(self.ctx, node).await?
            } else {
                true
            };

            if pass {
                let Some(next_node) = node.get_next_node_by_edge(edge) else {
                    anyhow::bail!("failed get next node");
                };
                let mut workflow_node = node.clone();
                workflow_node.reached_edge = Some(edge.clone());
                workflow_node.current_node = next_node.clone();

                self.flow_next(workflow_node).await?;
            }
        }

        Ok(())
    }

    pub async fn handle_end_event(&self, node: &WorkflowNode) -> Result<()> {
        // update node status
        WorkflowProcessNode::update_many()
            .set(workflow_process_node::ActiveModel {
                node_status: Set(NodeStatus::End.to_string()),
                ..Default::default()
            })
            .filter(workflow_process_node::Column::ProcessId.eq(&node.process_id))
            .filter(workflow_process_node::Column::NodeId.eq(&node.current_node.id))
            .exec(&self.ctx.db)
            .await?;

        // update process status
        WorkflowProcess::update_many()
            .set(workflow_process::ActiveModel {
                process_status: Set(ProcessStatus::End.to_string()),
                current_node_status: Set(NodeStatus::End.to_string()),
                ..Default::default()
            })
            .filter(workflow_process::Column::ProcessId.eq(&node.process_id))
            .exec(&self.ctx.db)
            .await?;

        Ok(())
    }

    pub async fn process_node(&self, mut node: WorkflowNode) -> Result<()> {
        node.flow_depth += 1;

        if !self.is_continue(&mut node).await? {
            return Ok(());
        }

        let ret = match node.current_node.node_type {
            NodeType::StartEvent => self.handle_start_event(&node).await,
            NodeType::ServiceTask => self.handle_service_task(&mut node).await,
            NodeType::EndEvent => self.handle_end_event(&node).await,
            NodeType::ExclusiveGateway => self.handle_exclusive_gateway(&node).await,
        };

        if let Err(e) = ret {
            error!(
                "failed handle workflow node, {e}, node: {:?}",
                serde_json::to_string(&node).unwrap_or_default()
            );
        }

        Ok(())
    }

    pub async fn update_node_status(&self, params: UpdateJobParams) -> Result<()> {
        let process_id = params.schedule_id;
        let run_id = params.run_id;
        let node_id = params.base_job.eid;
        let bind_ip = params.bind_ip;

        let mut output = params.stdout.unwrap_or_default();
        if let Some(e) = params.stderr
            && e != ""
        {
            output = format!("{e}\n{output}");
        }

        let Some(run_status) = params.run_status else {
            anyhow::bail!(
                "none run status, process_id:{}, node_id:{}, run_id:{}",
                process_id,
                node_id,
                run_id,
            );
        };

        let mut cond = workflow_process_node_task::Column::ProcessId
            .eq(&process_id)
            .and(workflow_process_node_task::Column::NodeId.eq(&node_id))
            .and(workflow_process_node_task::Column::BindIp.eq(&bind_ip));

        if run_status == RunStatus::Running {
            cond = cond.and(
                workflow_process_node_task::Column::TaskStatus.eq(RunStatus::Prepare.to_string()),
            );
        }

        WorkflowProcessNodeTask::update_many()
            .set(workflow_process_node_task::ActiveModel {
                task_status: Set(run_status.to_string()),
                exit_code: params.exit_code.map_or(NotSet, |v| Set(v.into())),
                exit_status: params.exit_status.map_or(NotSet, |v| Set(v)),
                output: Set(output),
                ..Default::default()
            })
            .filter(cond)
            .exec(&self.ctx.db)
            .await?;

        let not_ok = WorkflowProcessNodeTask::find()
            .filter(workflow_process_node_task::Column::ProcessId.eq(&process_id))
            .filter(workflow_process_node_task::Column::NodeId.eq(&node_id))
            .filter(workflow_process_node_task::Column::TaskStatus.ne(RunStatus::Stop.to_string()))
            .one(&self.ctx.db)
            .await?;
        if not_ok.is_none() {
            WorkflowProcessNode::update_many()
                .set(workflow_process_node::ActiveModel {
                    node_status: Set(NodeStatus::End.to_string()),
                    ..Default::default()
                })
                .filter(workflow_process_node::Column::ProcessId.eq(&process_id))
                .filter(workflow_process_node::Column::NodeId.eq(&node_id))
                .filter(workflow_process_node::Column::RunId.eq(&run_id))
                .exec(&self.ctx.db)
                .await?;

            // WorkflowProcess::update_many()
            //     .set(workflow_process::ActiveModel {
            //         current_node_id: Set(node_id.to_string()),
            //         current_node_status: Set(NodeStatus::End.to_string()),
            //         current_run_id: Set(run_id.to_string()),
            //         ..Default::default()
            //     })
            //     .filter(workflow_process::Column::ProcessId.eq(&process_id))
            //     .exec(&self.ctx.db)
            //     .await?;

            let Some(fields) = params.fields else {
                anyhow::bail!("fields is none");
            };

            let current_node =
                serde_json::from_value::<WorkflowNode>(fields["workflow_node"].clone())?;

            let next_point = current_node.get_next_nodes()?;

            for point in next_point {
                let mut next_node = current_node.clone();
                next_node.reached_edge = Some(point.0.to_owned());
                next_node.current_node = point.1.to_owned();
                self.flow_next(next_node).await?;
            }
        }
        Ok(())
    }

    pub async fn start_process(
        &self,
        user_info: &UserInfo,
        workflow_id: u64,
        version_id: u64,
        timer_id: Option<u64>,
        process_name: String,
        process_args: Option<WorkflowProcessArgs>,
    ) -> Result<String> {
        let version_record = WorkflowVersion::find()
            .filter(workflow_version::Column::WorkflowId.eq(workflow_id))
            .filter(workflow_version::Column::Id.eq(version_id))
            .one(&self.ctx.db)
            .await?
            .ok_or(anyhow!("not found workflow version record"))?;

        let nodes: Vec<NodeConfig> =
            serde_json::from_value(version_record.nodes.ok_or(anyhow!("invalid nodes data"))?)?;
        let edges: Vec<EdgeConfig> =
            serde_json::from_value(version_record.edges.ok_or(anyhow!("invalid edges data"))?)?;

        let no_input_target = process_args.is_none()
            || !process_args.as_ref().is_some_and(|v| {
                let default_target = v.default_target.as_ref();
                v.nodes.as_ref().is_some_and(|v| {
                    v.iter().any(|v| {
                        if v.node_id == v.node_id {
                            return v.target.as_ref().is_some_and(|v| v.len() > 0);
                        }
                        return false;
                    })
                }) || default_target.is_some_and(|v| v.len() > 0)
            });

        for v in nodes.iter() {
            match v.task {
                Task::Standard(ref standard_job) => {
                    let no_config_target = standard_job.target.is_none()
                        || !standard_job.target.as_ref().is_some_and(|v| v.len() > 0);

                    if no_config_target && no_input_target {
                        anyhow::bail!("No server node is set for executing tasks");
                    }
                }
                Task::Custom(ref custom_job) => {
                    let no_config_target = custom_job.target.is_none()
                        || !custom_job.target.as_ref().is_some_and(|v| v.len() > 0);

                    if no_config_target && no_input_target {
                        anyhow::bail!("No server node is set for executing tasks");
                    }
                }
                Task::None => (),
            };
        }

        let start_node = nodes
            .iter()
            .find(|&v| v.node_type == NodeType::StartEvent)
            .ok_or(anyhow!("not found start node"))?
            .to_owned();
        let curr_node_id = start_node.id.clone();

        let (process_id, run_id) = (nanoid::nanoid!(), nanoid::nanoid!());

        self.flow_next(WorkflowNode {
            created_user: user_info.username.clone(),
            process_id: process_id.clone(),
            run_id,
            origin_nodes: nodes,
            origin_edges: edges,
            process_args: process_args.clone(),
            flow_depth: 0,
            actual_args: None,
            reached_edge: None,
            current_node: start_node,
        })
        .await?;

        WorkflowProcess::insert(workflow_process::ActiveModel {
            process_id: Set(process_id.clone()),
            process_name: Set(process_name),
            workflow_id: Set(workflow_id),
            version_id: Set(version_id),
            process_args: Set(process_args.map(|v| serde_json::to_value(v)).transpose()?),
            process_status: Set(ProcessStatus::Running.to_string()),
            current_node_id: Set(curr_node_id),
            current_node_status: Set(NodeStatus::Prepare.to_string()),
            created_user: Set(user_info.username.clone()),
            timer_id: timer_id.map_or(NotSet, Set),
            ..Default::default()
        })
        .exec(&self.ctx.db)
        .await?;

        Ok(process_id)
    }

    pub async fn recv_flow_msg(
        &self,
        mut cb: impl Sync
        + Send
        + FnMut(String, WorkflowNode) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>,
    ) -> Result<String> {
        let redis_client = self.ctx.redis();
        let mut conn = redis_client.get_multiplexed_async_connection().await?;

        let ret: String = conn
            .xgroup_create_mkstream(Self::WORKFLOW_TOPIC, Self::CONSUMER_GROUP, "$")
            .await
            .map_or_else(
                |e| {
                    if e.code() != Some("BUSYGROUP") {
                        warn!("failed create workflow stream group - {}", e);
                    }
                    "".to_string()
                },
                |v| v,
            );

        info!("create workflow stream group {}", ret);

        let opts = StreamReadOptions::default()
            .group(Self::CONSUMER_GROUP, local_ip()?.to_string())
            .block(100)
            .count(100);

        loop {
            let ret: StreamReadReply = conn
                .xread_options(&[Self::WORKFLOW_TOPIC], &[">"], &opts)
                .await?;

            if let Err(e) = conn
                .xtrim::<_, u64>(Self::WORKFLOW_TOPIC, StreamMaxlen::Equals(5000))
                .await
            {
                error!("failed to trim workflow stream - {e}");
            };

            for stream_key in ret.keys {
                let msg_key = stream_key.key;

                for stream_id in stream_key.ids {
                    for (k, v) in stream_id.map {
                        let ret = match from_redis_value::<WorkflowNode>(&v) {
                            Ok(msg) => cb(k, msg).await,
                            Err(e) => {
                                error!("failed to parse workflow msg - {e}");
                                Ok(())
                            }
                        };

                        if let Err(e) = ret {
                            error!("failed to handle workflow msg - {e}");
                        }

                        let _: i32 = conn
                            .xack(
                                msg_key.clone(),
                                Self::CONSUMER_GROUP,
                                &[stream_id.id.clone()],
                            )
                            .await
                            .map_or_else(
                                |v| {
                                    error!("faile to exec workflow msg xack - {}", v);
                                    0
                                },
                                |v| v,
                            );
                    }
                }
            }
        }
    }

    pub async fn delete_workflow(&self, user_info: &UserInfo, workflow_id: u64) -> Result<u64> {
        let ret = Workflow::update_many()
            .set(workflow::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(workflow::Column::Id.eq(workflow_id))
            .exec(&self.ctx.db)
            .await?;

        TagResource::delete_many()
            .filter(tag_resource::Column::ResourceType.eq(ResourceType::Workflow.to_string()))
            .filter(tag_resource::Column::ResourceId.eq(workflow_id))
            .exec(&self.ctx.db)
            .await?;
        Ok(ret.rows_affected)
    }

    pub async fn delete_process(
        &self,
        user_info: &UserInfo,
        username: Option<String>,
        workflow_id: Option<u64>,
        process_id: Option<String>,
        team_id: Option<u64>,
        is_soft: Option<bool>,
    ) -> Result<u64> {
        if is_soft == Some(true) {
            let ret = WorkflowProcess::update_many()
                .set(workflow_process::ActiveModel {
                    is_deleted: Set(true),
                    deleted_at: Set(Some(Local::now())),
                    deleted_by: Set(user_info.username.clone()),
                    ..Default::default()
                })
                .apply_if(username, |q, v| {
                    q.filter(workflow_process::Column::CreatedUser.eq(v))
                })
                .apply_if(workflow_id, |q, v| {
                    q.filter(workflow_process::Column::WorkflowId.eq(v))
                })
                .apply_if(process_id, |q, v| {
                    q.filter(workflow_process::Column::ProcessId.eq(v))
                })
                .apply_if(team_id, |q, v| {
                    q.filter(
                        workflow_process::Column::WorkflowId.in_subquery(
                            Query::select()
                                .column(workflow::Column::Id)
                                .from(workflow::Entity)
                                .and_where(workflow::Column::TeamId.eq(v))
                                .to_owned(),
                        ),
                    )
                })
                .exec(&self.ctx.db)
                .await?
                .rows_affected;

            Ok(ret)
        } else {
            let ret = WorkflowProcess::delete_many()
                .apply_if(username, |q, v| {
                    q.filter(workflow_process::Column::CreatedUser.eq(v))
                })
                .apply_if(workflow_id, |q, v| {
                    q.filter(workflow_process::Column::WorkflowId.eq(v))
                })
                .apply_if(process_id, |q, v| {
                    q.filter(workflow_process::Column::ProcessId.eq(v))
                })
                .apply_if(team_id, |q, v| {
                    q.filter(
                        workflow_process::Column::WorkflowId.in_subquery(
                            Query::select()
                                .column(workflow::Column::Id)
                                .from(workflow::Entity)
                                .and_where(workflow::Column::TeamId.eq(v))
                                .to_owned(),
                        ),
                    )
                })
                .exec(&self.ctx.db)
                .await?
                .rows_affected;
            WorkflowProcessNode::delete_many()
                .filter(
                    workflow_process_node::Column::ProcessId.not_in_subquery(
                        Query::select()
                            .column(workflow_process::Column::ProcessId)
                            .from(workflow_process::Entity)
                            .to_owned(),
                    ),
                )
                .exec(&self.ctx.db)
                .await?;

            WorkflowProcessEdge::delete_many()
                .filter(
                    workflow_process_edge::Column::ProcessId.not_in_subquery(
                        Query::select()
                            .column(workflow_process::Column::ProcessId)
                            .from(workflow_process::Entity)
                            .to_owned(),
                    ),
                )
                .exec(&self.ctx.db)
                .await?;

            WorkflowProcessNodeTask::delete_many()
                .filter(
                    workflow_process_node_task::Column::ProcessId.not_in_subquery(
                        Query::select()
                            .column(workflow_process::Column::ProcessId)
                            .from(workflow_process::Entity)
                            .to_owned(),
                    ),
                )
                .exec(&self.ctx.db)
                .await?;

            Ok(ret)
        }
    }

    pub async fn delete_version(
        &self,
        user_info: &UserInfo,
        workflow_id: u64,
        version_id: u64,
    ) -> Result<u64> {
        let ret = WorkflowVersion::update_many()
            .set(workflow_version::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(workflow_version::Column::WorkflowId.eq(workflow_id))
            .filter(workflow_version::Column::Id.eq(version_id))
            .exec(&self.ctx.db)
            .await?;
        Ok(ret.rows_affected)
    }

    pub fn check_timer_expr(timezone: &str, expr: &str) -> Result<Vec<String>> {
        let parsed_expr = match CronParser::builder()
            .seconds(croner::parser::Seconds::Required)
            .dom_and_dow(true)
            .build()
            .parse(&expr)
        {
            Ok(_) => expr.to_string(),
            Err(e1) => match english_to_cron::str_cron_syntax(expr) {
                Ok(english_to_cron) => {
                    if english_to_cron != expr {
                        if english_to_cron == "0 * * * * ? *" {
                            anyhow::bail!("failed parse {} to cron expr, {}", expr, e1.to_string())
                        } else {
                            // english-to-cron adds the year field which we can't put off (currently)
                            let cron = english_to_cron
                                .split(' ')
                                .take(6)
                                .collect::<Vec<_>>()
                                .join(" ");
                            cron
                        }
                    } else {
                        expr.to_string()
                    }
                }
                Err(e2) => {
                    anyhow::bail!(
                        "failed parse cron expr, 1.{}, 2.{}",
                        e1.to_string(),
                        e2.to_string()
                    )
                }
            },
        };

        let parsed_cron = match Cron::from_str(&parsed_expr) {
            Err(e) => anyhow::bail!("failed build cron, {}", e.to_string()),
            Ok(v) => v,
        };

        let mut now = Local::now();
        let mut next_exec_times: Vec<String> = vec![];

        for _ in 0..10 {
            let next_time = match parsed_cron.find_next_occurrence(&now, false) {
                Err(e) => anyhow::bail!("failed find next execution time, {}", e.to_string()),
                Ok(v) => {
                    now = v.clone();
                    match timezone {
                        "local" => v
                            .with_timezone(&Local)
                            .format("%Y/%m/%d %H:%M:%S")
                            .to_string(),
                        "utc" | _ => v
                            .with_timezone(&Utc)
                            .format("%Y/%m/%d %H:%M:%S")
                            .to_string(),
                    }
                }
            };
            next_exec_times.push(next_time);
        }

        Ok(next_exec_times)
    }

    pub async fn save_timer(
        &self,
        _user_info: &UserInfo,
        active_model: workflow_timer::ActiveModel,
    ) -> Result<u64> {
        let active_model = active_model.save(&self.ctx.db).await?;
        Ok(active_model.id.as_ref().to_owned())
    }

    pub async fn delete_timer(&self, user_info: &UserInfo, id: u64) -> Result<u64> {
        let Some(data) = WorkflowTimer::find()
            .filter(workflow_timer::Column::Id.eq(id))
            .one(&self.ctx.db)
            .await?
        else {
            anyhow::bail!("not found");
        };
        if data.is_active {
            anyhow::bail!("cannot delete running timer");
        }

        let ret = WorkflowTimer::update_many()
            .set(workflow_timer::ActiveModel {
                is_deleted: Set(true),
                deleted_at: Set(Some(Local::now())),
                deleted_by: Set(user_info.username.clone()),
                ..Default::default()
            })
            .filter(workflow_timer::Column::Id.eq(id))
            .exec(&self.ctx.db)
            .await?;
        Ok(ret.rows_affected)
    }
}
