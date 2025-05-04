#[macro_export]
macro_rules! api_response {
    ($type:ty) => {
        poem::Result<crate::response::ApiStdResponse<$type>>
    };
}

#[macro_export]
macro_rules! return_ok {
    ($data:expr) => {
        return Ok(poem_openapi::payload::Json(crate::response::StdResponse {
            code: 20000,
            data: Some($data),
            msg: "success".to_string(),
        }))
    };
}

#[macro_export]
macro_rules! return_err {
    ($msg:expr) => {{
        let mut e = poem::Error::from_string($msg, poem::http::StatusCode::OK);
        e.set_data(50001i32);
        return Err(e);
    }};
}

/// convert DateTime<Local> to local time(String)
#[macro_export]
macro_rules! local_time {
    ($time:expr) => {
        $time
            .with_timezone(&chrono::Local)
            .naive_local()
            .to_string()
    };
}
#[macro_export]
macro_rules! time_format {
    ($time:expr, $format:expr) => {
        $time
            // .with_timezone(&chrono::Local)
            .naive_local()
            .format($format)
            .to_string()
    };
}

#[macro_export]
macro_rules! default_string {
    ($v:expr, $default:expr) => {
        $v.clone().map_or($default.to_string(), |v| v)
    };
}

#[macro_export]
macro_rules! default_local_time {
    ($time:expr) => {
        $time.clone().map_or("".to_string(), |v| {
            v.with_timezone(&chrono::Local).naive_local().to_string()
        })
    };
}

#[macro_export]
macro_rules! return_err_to_wsconn {
    ($client:expr, $err_msg:expr) => {
        if let Err(e) = $client
            .send(poem::web::websocket::Message::Text(format!(
                "\r\n\x1b[31m{}",
                $err_msg
            )))
            .await
        {
            error!("failed send message to ws connection - {e}");
        }
        return;
    };
}
