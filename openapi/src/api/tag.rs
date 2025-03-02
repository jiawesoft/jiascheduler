use poem::web::Data;
use poem_openapi::OpenApi;
use types::SaveTagResp;

use crate::{api_response, response::ApiStdResponse, state::AppState};

pub mod types {
    use poem_openapi::Object;
    use serde::{Deserialize, Serialize};

    #[derive(Object)]
    pub struct SaveTagReq {
        pub id: Option<u64>,
        pub tag_name: String,
    }

    #[derive(Object)]
    pub struct SaveTagResp {
        pub id: Option<u64>,
        pub tag_name: String,
    }
}

pub struct TagApi;

#[OpenApi(prefix_path="/tag", tag = super::Tag::Tag)]
impl TagApi {
    #[oai(path = "/save", method = "post")]
    pub async fn save_job(&self, _state: Data<&AppState>) -> api_response!(SaveTagResp) {
        todo!()
    }
}
