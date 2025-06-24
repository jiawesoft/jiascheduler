use poem::{web::Data, Endpoint, EndpointExt};
use poem_openapi::{
    param::{Header, Query},
    payload::Json,
    OpenApi,
};
use types::{BindTagResp, UnbindTagResp};

use crate::{
    api_response,
    logic::{self, types::ResourceType},
    middleware, return_ok,
    state::AppState,
};

pub mod types {
    use poem_openapi::{Enum, Object};
    use serde::{Deserialize, Serialize};

    #[derive(Object, Deserialize, Serialize)]
    pub struct BindTagReq {
        pub resource_id: u64,
        pub resource_type: ResourceType,
        pub tag_name: String,
    }

    #[derive(Serialize, Default, Deserialize, Enum)]
    pub enum ResourceType {
        #[default]
        #[oai(rename = "job")]
        Job,
        #[oai(rename = "instance")]
        Instance,
        #[oai(rename = "workflow")]
        Workflow,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct BindTagResp {
        pub result: u64,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct UnbindTagReq {
        pub resource_id: u64,
        pub resource_type: ResourceType,
        pub tag_id: u64,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct UnbindTagResp {
        pub result: u64,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct CountTagResp {
        pub list: Vec<TagCount>,
    }

    #[derive(Object, Deserialize, Serialize)]
    pub struct TagCount {
        pub tag_id: u64,
        pub tag_name: String,
        pub total: i64,
    }
}

fn set_middleware(ep: impl Endpoint) -> impl Endpoint {
    ep.with(middleware::TeamPermissionMiddleware)
}

pub struct TagApi;

#[OpenApi(prefix_path="/tag", tag = super::Tag::Tag)]
impl TagApi {
    #[oai(path = "/bind_tag", method = "post", transform = "set_middleware")]
    pub async fn bind_tag(
        &self,
        user_info: Data<&logic::types::UserInfo>,
        state: Data<&AppState>,
        Json(req): Json<types::BindTagReq>,
    ) -> api_response!(BindTagResp) {
        let svc = state.service();
        let resource_type = match req.resource_type {
            types::ResourceType::Job => ResourceType::Job,
            types::ResourceType::Instance => ResourceType::Instance,
            types::ResourceType::Workflow => ResourceType::Workflow,
        };

        let ret = svc
            .tag
            .bind_tag(&user_info, &req.tag_name, resource_type, req.resource_id)
            .await?;

        return_ok!(BindTagResp { result: ret });
    }

    #[oai(path = "/unbind_tag", method = "post", transform = "set_middleware")]
    pub async fn unbind_tag(
        &self,
        user_info: Data<&logic::types::UserInfo>,
        state: Data<&AppState>,
        Json(req): Json<types::UnbindTagReq>,
    ) -> api_response!(UnbindTagResp) {
        let svc = state.service();
        let resource_type = match req.resource_type {
            types::ResourceType::Job => ResourceType::Job,
            types::ResourceType::Instance => ResourceType::Instance,
            types::ResourceType::Workflow => ResourceType::Workflow,
        };
        let ret = svc
            .tag
            .unbind_tag(&user_info, req.tag_id, resource_type, vec![req.resource_id])
            .await?;
        return_ok!(UnbindTagResp { result: ret });
    }

    #[oai(path = "/count_resource", method = "get", transform = "set_middleware")]
    pub async fn count_resource(
        &self,
        user_info: Data<&logic::types::UserInfo>,
        state: Data<&AppState>,
        Query(resource_type): Query<types::ResourceType>,
        #[oai(name = "X-Team-Id")] Header(team_id): Header<Option<u64>>,
    ) -> api_response!(types::CountTagResp) {
        let svc = state.service();
        let resource_type = match resource_type {
            types::ResourceType::Job => ResourceType::Job,
            types::ResourceType::Instance => ResourceType::Instance,
            types::ResourceType::Workflow => ResourceType::Workflow,
        };

        let search_username =
            if state.can_manage_job(&user_info.user_id).await? || team_id.is_some() {
                None
            } else {
                Some(user_info.username.clone())
            };

        let ret = svc
            .tag
            .count_resource(&user_info, resource_type, team_id, search_username)
            .await?;

        let list: Vec<types::TagCount> = ret
            .into_iter()
            .map(|v| types::TagCount {
                tag_id: v.tag_id,
                tag_name: v.tag_name,
                total: v.total,
            })
            .collect();

        let resp = types::CountTagResp { list };

        return_ok!(resp);
    }
}
