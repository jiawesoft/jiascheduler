use anyhow::{anyhow, Result};
use poem::{error::ResponseError, http::StatusCode, Error as PError, IntoResponse};
use poem_openapi::payload::Json;
use std::{error::Error as StdError, ops::Deref};
use thiserror::Error;

use crate::response::StdResponse;

#[allow(unused)]
#[derive(Error, Debug)]
pub enum BizError {
    #[error("无效的请求参数 `{0}`")]
    InvalidReqParams(String),
    #[error("`{0}` 无效的JSON格式 - {1}")]
    InvalidJSON(&'static str, anyhow::Error),
}

pub struct BizErr(PError);

impl From<BizErr> for PError {
    fn from(value: BizErr) -> Self {
        value.0
    }
}

impl BizErr {
    pub fn new<S: Into<String>>(msg: S, code: i32) -> Self {
        let mut e = PError::from_string(msg.into(), StatusCode::OK);
        e.set_data(code);
        BizErr(e)
    }

    pub fn with_error(mut self, err: impl StdError + Send + Sync + 'static) -> Self {
        self.0
            .set_error_message(format!("{}: {}", self.0.to_string(), err.to_string()));
        self
    }

    pub fn with_msg(mut self, msg: impl Into<String>) -> Self {
        self.0
            .set_error_message(format!("{}: {}", self.0.to_string(), msg.into()));
        self
    }

    pub fn error(self) -> Result<()> {
        Err(anyhow!(self.0))
    }
}

impl Deref for BizErr {
    type Target = PError;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

macro_rules! define_biz_error {
    ($($(#[$docs:meta])* ($name:ident, $code:expr, $msg:expr);)*) => {
        $(
            $(#[$docs])*
            #[allow(non_snake_case)]
            #[inline]
            #[allow(unused)]
            pub fn $name() -> BizErr {
                // let mut e= PError::from_string($msg, StatusCode::OK);
                // e.set_data($code);
                // BizErr(e)
                BizErr::new($msg, $code)
            }
        )*

    };


}

define_biz_error!(
    (InvalidJSON, 50003, "Invalid JSON format");
    (BizError, 50000, "Internal error");
    (InvalidUser, 50004, "Invalid username or passowrd");
    (NoPermission, 50005, "This operation is not allowed");
);

impl ResponseError for BizError {
    fn status(&self) -> StatusCode {
        StatusCode::OK
    }
}

pub async fn custom_error(e: PError) -> impl IntoResponse {
    let mut code = e.status().as_u16() as i32;
    let mut status_code = e.status();
    let mut msg = e.to_string();
    if code == 500 {
        status_code = StatusCode::OK;
        code = 50000
    }

    if code == 400 {
        status_code = StatusCode::OK;
        code = 50400
    }

    if msg.contains("Duplicate entry") {
        msg = "record already exists, please do not add it again".to_string()
    }

    let code = e.data::<i32>().unwrap_or(&code).to_owned();
    Json(StdResponse::<bool> {
        code,
        data: None,
        msg,
    })
    .with_status(status_code)
}
