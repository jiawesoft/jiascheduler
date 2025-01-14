use crate::{
    logic::{self},
    state::AppState,
};
use poem::{web::Json, Endpoint, IntoResponse, Middleware, Request, Response, Result};

pub struct TeamPermissionMiddleware;

impl<E: Endpoint> Middleware<E> for TeamPermissionMiddleware {
    type Output = TeamPermissionMiddlewareEndpoint<E>;

    fn transform(&self, ep: E) -> Self::Output {
        TeamPermissionMiddlewareEndpoint { ep }
    }
}

pub struct TeamPermissionMiddlewareEndpoint<E> {
    ep: E,
}

// #[async_trait::async_trait]
impl<E> Endpoint for TeamPermissionMiddlewareEndpoint<E>
where
    E: Endpoint,
{
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        let resp = Json(serde_json::json! ({
            "code": 50403,
            "msg": "No permission to access the team's jobs",
        }))
        .into_response();

        let team_id = match req.header("X-Team-Id").map(str::parse::<u64>).transpose() {
            Ok(v) => v,
            Err(e) => {
                return Ok(Json(serde_json::json! ({
                    "code": 50000,
                    "msg": e.to_string(),
                }))
                .into_response())
            }
        };

        if team_id.is_none() {
            return self.ep.call(req).await.map(IntoResponse::into_response);
        }

        let user_info: &logic::types::UserInfo =
            req.extensions().get().expect("not init user info");
        let state: &AppState = req.extensions().get().expect("not init state");

        let ok = state
            .service()
            .team
            .can_write_job(team_id, &user_info.user_id)
            .await?;
        if !ok {
            return Ok(resp);
        }
        self.ep.call(req).await.map(IntoResponse::into_response)
    }
}
