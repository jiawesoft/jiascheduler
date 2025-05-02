use std::collections::HashMap;

use anyhow::Result;

use automate::scheduler::types::RunStatus;
use sea_orm::{
    ColumnTrait, DbBackend, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Statement,
};
use sea_query::{Alias, Expr};
use serde::{Deserialize, Serialize};
use sql_builder::{SqlBuilder, bind::Bind};

use super::{
    JobLogic,
    types::{self, BundleScriptRecord, JobStatSummary, RunResultSummary},
};

use crate::{
    entity::{job, job_exec_history, job_running_status, job_schedule_history, prelude::*},
    logic,
};

use utils::local_time;

#[derive(Debug, FromQueryResult)]
pub struct JobExecCount {
    eid: String,
    total: i64,
    is_exec_succ: bool,
}
#[derive(Debug, FromQueryResult)]
struct BundleScriptExecCount {
    eid: String,
    result: bool,
    exit_code: i64,
    is_eval_err: bool,
    total: i64,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
pub struct FilterScheduleAndRunTime {
    pub eid: String,
    pub schedule_id: String,
    pub run_id: String,
}

impl<'a> JobLogic<'a> {
    async fn get_job_exec_count_result(
        &self,
        schedule_id: String,
        run_id: Option<String>,
    ) -> Result<Vec<JobExecCount>> {
        // const TPL: &'static str = r#"
        //     select
        //         eid,
        //         count(1) total,
        //         if(exit_status='',true,false) is_exec_succ
        //     from
        //         job_exec_history
        //     where
        //         schedule_id = ? {}
        //     group by
        //         eid,
        //         is_exec_succ;
        // "#;
        // if let Some(run_time) = run_time {
        //     let sql = format!(TPL, "and DATE_FORMAT(created_time, '%Y-%m-%d %H:%i')");
        // }

        let ret = JobExecHistory::find()
            .select_only()
            .column(job_exec_history::Column::Eid)
            .column_as(job_exec_history::Column::Eid.count(), "total")
            .expr_as(
                Expr::case(Expr::col(Alias::new("exit_code")).eq(0), true).finally(false),
                "is_exec_succ",
            )
            .filter(job_exec_history::Column::ScheduleId.eq(schedule_id))
            .group_by(Expr::col(Alias::new("eid")))
            // .apply_if(run_time, |query, v| {
            //     query.filter(Expr::cust(
            //         "`created_time` BETWEEN '2025-03-15 22:14:00' AND '2025-03-15 22:14:59'",
            //     ))
            // })
            // .apply_if(run_time, |query, v| {
            //     query.filter(
            //         Expr::custom_keyword(Alias::new(
            //             "DATE_FORMAT(`job_exec_history`.`start_time`, '%Y-%m-%d %H:%i')",
            //         ))
            //         .eq(v),
            //     )
            // })
            .apply_if(run_id, |q, v| {
                q.filter(job_exec_history::Column::RunId.eq(v))
            })
            .group_by(Expr::col(Alias::new("is_exec_succ")))
            .into_model::<JobExecCount>()
            .all(&self.ctx.db)
            .await?;

        // let ret =    JobExecCount::find_by_statement(Statement::from_string(
        //         sea_orm::DatabaseBackend::MySql,
        //         "SELECT `job_exec_history`.`eid`, COUNT(`job_exec_history`.`eid`) AS `total`, (CASE WHEN (`exit_code` = 0) THEN TRUE ELSE FALSE END) AS `is_exec_succ` FROM `job_exec_history` WHERE `job_exec_history`.`schedule_id` = 's-UUNUXT95Iy' AND DATE_FORMAT(`created_time`, '%Y-%m-%d %H:%i') = '2025-03-15 22:14' GROUP BY `eid`, `is_exec_succ`"))
        //     .all(&self.ctx.db).await?;

        // let ret: Vec<JobExecCount> = JobExecCount::find_by_statement(
        //     Statement::from_sql_and_values(DbBackend::MySql, sql, [schedule_id.into()]),
        // )
        // .all(&self.ctx.db)
        // .await?;

        Ok(ret)
    }

    async fn get_bundle_script_exec_count_result(
        &self,
        schedule_id: String,
        run_id: Option<String>,
    ) -> Result<Vec<BundleScriptExecCount>> {
        let mut sb = SqlBuilder::select_from("job_exec_history jeh")
            .and_table(
                r#"json_table(jeh.bundle_script_result , '$[*]' columns(
                    eid varchar(20) path '$.eid',
                    exit_code int path '$.exit_code',
                    eval_err varchar(500) path '$.eval_err',
                    result bool path '$.result')) bst"#,
            )
            .fields(&[
                "bst.eid",
                "count(1) total",
                "bst.exit_code",
                "if(bst.eval_err='', false,true) is_eval_err",
                "bst.result",
            ])
            .and_where("jeh.schedule_id = ?".bind(&schedule_id))
            .to_owned();
        if let Some(run_id) = run_id {
            sb.and_where("jeh.run_id=?".bind(&run_id));
        }
        sb.group_by("bst.eid,bst.exit_code,bst.result,eval_err");

        let sql = sb.sql()?;
        let ret: Vec<BundleScriptExecCount> =
            BundleScriptExecCount::find_by_statement(Statement::from_string(DbBackend::MySql, sql))
                .all(&self.ctx.db)
                .await?;
        Ok(ret)
    }

    pub async fn get_latest_schedule(
        &self,
        eid: String,
        schedule_id: Option<String>,
        run_id: Option<String>,
    ) -> Result<Option<(job_schedule_history::Model, String, String)>> {
        let (schedule_id, latest_exec_time, run_id) = if let Some(schedule_id) = schedule_id {
            match JobExecHistory::find()
                .filter(job_exec_history::Column::Eid.eq(eid.clone()))
                .filter(job_exec_history::Column::ScheduleId.eq(schedule_id.clone()))
                .apply_if(run_id, |q, v| {
                    q.filter(job_exec_history::Column::RunId.eq(v))
                })
                .order_by(job_exec_history::Column::Id, sea_orm::Order::Desc)
                .one(&self.ctx.db)
                .await?
            {
                Some(v) => (schedule_id, local_time!(v.created_time), v.run_id.clone()),
                None => return Ok(None),
            }
        } else {
            match JobExecHistory::find()
                .filter(job_exec_history::Column::Eid.eq(eid.clone()))
                .apply_if(run_id, |q, v| {
                    q.filter(job_exec_history::Column::RunId.eq(v))
                })
                .order_by(job_exec_history::Column::Id, sea_orm::Order::Desc)
                .one(&self.ctx.db)
                .await?
            {
                Some(v) => (v.schedule_id, local_time!(v.created_time), v.run_id.clone()),
                None => return Ok(None),
            }
        };

        let ret = JobScheduleHistory::find()
            .filter(job_schedule_history::Column::ScheduleId.eq(schedule_id))
            .one(&self.ctx.db)
            .await?
            .map(|v| (v, latest_exec_time, run_id));

        Ok(ret)
    }

    /// can optimize performance
    pub async fn get_summary(&self, user_info: &logic::types::UserInfo) -> Result<JobStatSummary> {
        let svc = self.ctx.service();
        let (search_user, team_ids) = if self.ctx.can_manage_job(&user_info.user_id).await? {
            (None, None)
        } else {
            let team_ids = svc
                .team
                .get_my_teams(&user_info.user_id)
                .await?
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<u64>>();
            (Some(user_info.username.clone()), Some(team_ids))
        };

        let mut summary: JobStatSummary = Default::default();
        summary.total = Job::find()
            .apply_if(search_user.clone(), |query, v| {
                query.filter(job::Column::CreatedUser.eq(v))
            })
            .count(&self.ctx.db)
            .await?;

        summary.running_num = JobRunningStatus::find()
            .join_rev(
                sea_orm::JoinType::LeftJoin,
                Job::belongs_to(JobRunningStatus)
                    .from(job::Column::Eid)
                    .to(job_running_status::Column::Eid)
                    .into(),
            )
            .apply_if(search_user.clone(), |query, v| {
                let mut cond = job::Column::CreatedUser.eq(v);
                if let Some(team_ids) = team_ids.clone() {
                    cond = cond.or(job::Column::TeamId.is_in(team_ids));
                }
                query.filter(cond)
            })
            .filter(job_running_status::Column::RunStatus.eq(RunStatus::Running.to_string()))
            .count(&self.ctx.db)
            .await?;

        summary.exec_succ_num = JobRunningStatus::find()
            .join_rev(
                sea_orm::JoinType::LeftJoin,
                Job::belongs_to(JobRunningStatus)
                    .from(job::Column::Eid)
                    .to(job_running_status::Column::Eid)
                    .into(),
            )
            .apply_if(search_user.clone(), |query, v| {
                let mut cond = job::Column::CreatedUser.eq(v);
                if let Some(team_ids) = team_ids.clone() {
                    cond = cond.or(job::Column::TeamId.is_in(team_ids));
                }
                query.filter(cond)
            })
            .filter(job_running_status::Column::ExitCode.eq(0))
            .count(&self.ctx.db)
            .await?;
        summary.exec_fail_num = JobRunningStatus::find()
            .join_rev(
                sea_orm::JoinType::LeftJoin,
                Job::belongs_to(JobRunningStatus)
                    .from(job::Column::Eid)
                    .to(job_running_status::Column::Eid)
                    .into(),
            )
            .apply_if(search_user.clone(), |query, v| {
                let mut cond = job::Column::CreatedUser.eq(v);
                if let Some(team_ids) = team_ids {
                    cond = cond.or(job::Column::TeamId.is_in(team_ids.clone()));
                }
                query.filter(cond)
            })
            .filter(job_running_status::Column::ExitCode.ne(0))
            .count(&self.ctx.db)
            .await?;
        Ok(summary)
    }

    pub async fn get_dashboard(
        &self,
        user_info: &logic::types::UserInfo,
        job_type: Option<String>,
        filter: Option<Vec<FilterScheduleAndRunTime>>,
    ) -> Result<Vec<types::JobRunResultStats>> {
        let svc = self.ctx.service();
        let (search_user, team_ids) = if self.ctx.can_manage_job(&user_info.user_id).await? {
            (None, None)
        } else {
            let team_ids = svc
                .team
                .get_my_teams(&user_info.user_id)
                .await?
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<u64>>();
            (Some(user_info.username.clone()), Some(team_ids))
        };

        let list = Job::find()
            .apply_if(search_user.clone(), |query, v| {
                let mut cond = job::Column::CreatedUser.eq(v);
                if let Some(team_ids) = team_ids.clone() {
                    cond = cond.or(job::Column::TeamId.is_in(team_ids));
                }
                query.filter(cond)
            })
            .apply_if(job_type, |query, v| {
                query.filter(job::Column::JobType.eq(v))
            })
            .filter(job::Column::DisplayOnDashboard.eq(true))
            .all(&self.ctx.db)
            .await?;

        let mut vals = Vec::new();

        for item in list {
            let matched = filter
                .as_ref()
                .and_then(|v| v.iter().find(|v| v.eid == item.eid));
            let (schedule_id, run_id) = matched
                .map(|v| (v.schedule_id.to_string(), v.run_id.to_string()))
                .unzip();

            if let Some((schedule_record, latest_exec_time, run_id)) = self
                .get_latest_schedule(item.eid.clone(), schedule_id, run_id)
                .await?
            {
                if schedule_record.job_type == "default" {
                    let count_result = self
                        .get_job_exec_count_result(schedule_record.schedule_id, Some(run_id))
                        .await?;
                    let mut stats = types::JobRunResultStats {
                        name: item.name.clone(),
                        eid: item.eid.clone(),
                        schedule_name: schedule_record.name.clone(),
                        ..Default::default()
                    };
                    let mut ret = RunResultSummary::default();
                    count_result.iter().for_each(|v| {
                        ret.eid = v.eid.clone();
                        ret.total += v.total;
                        if v.is_exec_succ {
                            ret.exec_succ_num += v.total
                        } else {
                            ret.exec_fail_num += v.total
                        }
                    });
                    ret.last_start_time = latest_exec_time.clone();
                    stats.last_start_time = latest_exec_time.clone();
                    stats.results = vec![ret];
                    vals.push(stats);
                } else {
                    let bundle_script: Vec<BundleScriptRecord> = serde_json::from_value(
                        item.bundle_script
                            .ok_or(anyhow::format_err!("cannot get bundle_sciprt"))?,
                    )?;

                    let mut stats = types::JobRunResultStats {
                        name: item.name.clone(),
                        eid: item.eid.clone(),
                        schedule_name: schedule_record.name.clone(),
                        ..Default::default()
                    };

                    let mut check_map = HashMap::new();

                    bundle_script.iter().for_each(|v| {
                        check_map.insert(
                            v.eid.clone(),
                            types::RunResultSummary {
                                name: v.name.clone(),
                                info: v.info.clone(),
                                eid: v.eid.clone(),
                                ..Default::default()
                            },
                        );
                    });
                    let count_result = self
                        .get_bundle_script_exec_count_result(
                            schedule_record.schedule_id,
                            Some(run_id),
                        )
                        .await?;
                    count_result.iter().for_each(|v| {
                        let bundle_stats = check_map.get_mut(&v.eid).unwrap();
                        bundle_stats.total += v.total;
                        if v.exit_code == 0 {
                            bundle_stats.exec_succ_num += v.total;
                            if v.result {
                                bundle_stats.check_succ_num += v.total
                            } else {
                                bundle_stats.check_fail_num += v.total
                            }
                        } else {
                            bundle_stats.exec_fail_num += v.total;
                        }
                        if v.is_eval_err {
                            bundle_stats.eval_fail_num += v.total
                        }
                        bundle_stats.last_start_time = latest_exec_time.clone();
                    });
                    stats.last_start_time = latest_exec_time;
                    stats.results = check_map.into_values().collect();
                    vals.push(stats);
                }
            }
        }

        Ok(vals)
    }
}

#[test]
fn test_sql_build() {
    let run_time = Some("2022-03-15 14:06:23");
    let schedule_id = "1647028539";
    let mut sb = SqlBuilder::select_from("job_schedule_history jsh")
        .and_table(
            r#"json_table(jeh.bundle_script_result , '$[*]' columns(
                    eid varchar(20) path '$.eid',
                    exit_code int path '$.exit_code',
                    eval_err varchar(500) path '$.eval_err',
                    result bool path '$.result')) bst"#,
        )
        .fields(&[
            "bst.eid",
            "count(1) total",
            "bst.exit_code",
            "if(bst.eval_err='', false,true) is_eval_err",
            "bst.result",
        ])
        .and_where("jeh.schedule_id = ?".bind(&schedule_id))
        .to_owned();
    if let Some(run_time) = run_time {
        sb.and_where("DATE_FORMAT(jeh.created_time, '%Y-%m-%d %H:%i')=?".bind(&run_time));
    }
    sb.group_by("bst.eid,bst.exit_code,bst.result,eval_err");

    let sql = sb.sql().unwrap();
    println!("sql:{:?}", sql);
}
