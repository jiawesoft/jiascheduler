/// Return api response
///
/// ```
/// // return success
/// return_response!()
///
/// // return success with data
/// return_response!("ok")
///
/// ```
///
///
#[macro_export]
macro_rules! return_response {
    () => {
        return poem::web::Json(serde_json::json!({
            "code":20000,
            "data":null,
            "msg":"success",
        }))
    };
    ($data:expr) => {
        return poem::web::Json(serde_json::json!({
            "code":20000,
            "data":Some($data),
            "msg":"success",
        }))
    };
    ($data:expr,$msg:expr) => {
        return poem::web::Json(serde_json::json!({
            "code":20000,
            "data":Some($data),
            "msg":$msg,
        }))
    };
    (code: $code:expr, $msg:expr) => {
        return poem::web::Json(serde_json::json!({
            "code":$code,
            "data": null,
            "msg":$msg,
        }))
    };
    (json: $val:expr) => {
        return poem::web::Json($val)
    };
}
