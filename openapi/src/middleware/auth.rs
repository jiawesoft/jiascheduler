use crate::logic::{types, user::UserLogic};
use poem::{
    session::Session, web::Json, Endpoint, IntoResponse, Middleware, Request, Response, Result,
};

pub struct AuthMiddleware;

impl<E: Endpoint> Middleware<E> for AuthMiddleware {
    type Output = AuthMiddlewareEndpoint<E>;

    fn transform(&self, ep: E) -> Self::Output {
        AuthMiddlewareEndpoint { ep }
    }
}

pub struct AuthMiddlewareEndpoint<E> {
    ep: E,
}

// #[async_trait::async_trait]
impl<E> Endpoint for AuthMiddlewareEndpoint<E>
where
    E: Endpoint,
{
    type Output = Response;

    async fn call(&self, mut req: Request) -> Result<Self::Output> {
        let login_resp = Json(serde_json::json! ({
            "code": 50401,
            "msg": "not login",
        }))
        .into_response();

        let sess: &Session = req.extensions().get().expect("not init session");

        if let Some(user_info) = sess.get::<types::UserInfo>(UserLogic::SESS_KEY) {
            req.extensions_mut().insert(user_info);
        } else {
            if vec!["/user/login", "/user/logout", "/migration/version/check"]
                .contains(&req.uri().path())
            {
                return self.ep.call(req).await.map(IntoResponse::into_response);
            }
            return Ok(login_resp);
        }
        self.ep.call(req).await.map(IntoResponse::into_response)
    }
}
