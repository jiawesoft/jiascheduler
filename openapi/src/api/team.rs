use poem::{session::Session, web::Data};
use poem_openapi::{param::Query, payload::Json, OpenApi};
use sea_orm::{ActiveValue::NotSet, Set};

use crate::{
    api_response, entity::team, local_time, logic, return_err, return_ok, state::AppState,
};

pub struct TeamApi;

mod types {
    use poem_openapi::Object;
    use serde::{Deserialize, Serialize};

    #[derive(Object, Serialize, Default)]
    pub struct SaveTeamReq {
        pub id: Option<u64>,
        pub name: String,
        pub info: Option<String>,
        pub user_id: Option<Vec<String>>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct SaveTeamResult {
        pub affected: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryTeamResp {
        pub total: u64,
        pub list: Vec<TeamRecord>,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct TeamRecord {
        pub id: u64,
        pub name: String,
        pub info: String,
        pub user_total: i64,
        pub created_time: String,
        pub created_user: String,
    }
}

#[OpenApi(prefix_path = "/team", tag = super::Tag::Team)]
impl TeamApi {
    #[oai(path = "/save", method = "post")]
    pub async fn save_team(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SaveTeamReq>,
    ) -> api_response!(types::SaveTeamResult) {
        let svc = state.service();
        if !svc
            .team
            .can_write_team(req.id, user_info.user_id.clone())
            .await?
        {
            return_err!("no permission");
        }

        let ret = svc
            .team
            .save_team(
                team::ActiveModel {
                    name: Set(req.name),
                    info: req.info.map_or(NotSet, |v| Set(v)),
                    created_user: Set(user_info.user_id.clone()),
                    ..Default::default()
                },
                req.user_id,
            )
            .await?;

        return_ok!(types::SaveTeamResult { affected: ret })
    }

    #[oai(path = "/list", method = "get")]
    pub async fn query_team(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        Query(id): Query<Option<u64>>,
        Query(default_id): Query<Option<u64>>,
        /// Team adminitor can query all team
        Query(user_id): Query<Option<String>>,
        Query(name): Query<Option<String>>,
        #[oai(
            default = "crate::api::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
        #[oai(
            default = "crate::api::default_page",
            validator(maximum(value = "10000"))
        )]
        Query(page): Query<u64>,
        user_info: Data<&logic::types::UserInfo>,
    ) -> api_response!(types::QueryTeamResp) {
        let svc = state.service();
        let user_id = if svc
            .team
            .can_write_job(None, user_info.user_id.clone())
            .await?
        {
            user_id
        } else {
            Some(user_info.user_id.clone())
        };

        let team_member_count = svc.team.count_team_member().await?;
        let ret = svc
            .team
            .query_team(name, user_id, id, default_id, page, page_size)
            .await?;

        let mut list: Vec<types::TeamRecord> = Vec::new();

        for v in ret.0 {
            list.push(types::TeamRecord {
                id: v.id,
                name: v.name,
                info: v.info,
                user_total: team_member_count
                    .get_by_team_id(v.id)
                    .map_or(0, |v| v.total),
                created_time: local_time!(v.created_time),
                created_user: v.created_user,
            });
        }
        return_ok!(types::QueryTeamResp { total: ret.1, list })
    }
}
