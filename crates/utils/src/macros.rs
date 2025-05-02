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
macro_rules! file_name {
    ($file:expr) => {
        PathBuf::from($file)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    };
}
