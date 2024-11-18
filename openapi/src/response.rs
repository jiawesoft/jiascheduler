use poem::{http::StatusCode, Error};
use poem_openapi::{
    payload::Json,
    types::{ParseFromJSON, ToJSON},
    Object,
};

use serde::{Deserialize, Serialize};

#[derive(Object, Serialize, Deserialize)]
pub struct StdResponse<T: ParseFromJSON + ToJSON> {
    pub code: i32,
    pub data: Option<T>,
    pub msg: String,
}

pub fn std_into_error(e: impl std::error::Error + Sync + Send + 'static) -> Error {
    let mut e = Error::new(e, StatusCode::OK);
    e.set_data(50001i32);
    e
}

pub fn anyhow_into_error(e: anyhow::Error) -> Error {
    let mut e = Error::from((StatusCode::OK, e));
    e.set_data(50001i32);
    e
}

pub type ApiStdResponse<T> = Json<StdResponse<T>>;
