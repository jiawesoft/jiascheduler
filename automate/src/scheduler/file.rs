// use crate::get_http_client;

use super::types::UploadFile;
use anyhow::Result;
use tokio::{
    fs::{create_dir_all, File},
    io::AsyncWriteExt,
};

const UPLOAD_DIR: &str = "/tmp/jiascheduler-agent";

pub async fn try_download_file(_host: String, file: Option<UploadFile>) -> Result<()> {
    let file = match file {
        Some(v) => v,
        None => return Ok(()),
    };

    let data = if let Some(data) = file.data {
        data
    } else {
        return Ok(());
    };

    // let client = get_http_client();
    // let data = client
    //     .get(format!("http://{}/file/get/{}", host, file.filename))
    //     .send()
    //     .await?
    //     .bytes()
    //     .await?;

    create_dir_all(UPLOAD_DIR).await?;
    let target_file = format!("{}/{}", UPLOAD_DIR, file.filename);
    let mut tmp_file = File::create(target_file.clone()).await?;
    tmp_file.write_all(&data).await?;
    Ok(())
}
