/// convert DateTime<Local> to local time(String)
#[macro_export]
macro_rules! local_time {
    ($time:expr) => {
        $time
            .with_timezone(&chrono::Local)
            .naive_local()
            .format("%Y/%m/%d %H:%M:%S")
            .to_string()
    };
}

#[macro_export]
macro_rules! file_name {
    ($file:expr) => {
        std::path::PathBuf::from($file)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    };
}
/// convert empty array to None
#[macro_export]
macro_rules! non_empty {
    ($arr:expr) => {
        $arr.clone()
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
    };
}

/// check if v is valid json
#[macro_export]
macro_rules! is_valid_json {
    ($v:expr) => {
        serde_json::from_str::<serde_json::Value>($v).is_ok()
    };
}
