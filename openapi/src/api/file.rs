use std::{
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};

use anyhow::anyhow;

use chrono::{DateTime, Utc};
use poem::{session::Session, web::Data, Result};
use poem_openapi::{
    param::{Path, Query},
    payload::{Attachment, AttachmentType, Json, PlainText},
    OpenApi,
};
use tokio::{
    fs::{self, create_dir_all, File},
    io::AsyncWriteExt,
};

use crate::{
    local_time,
    logic::{
        self,
        ssh::{ConnectParams, Session as SshSession},
    },
    response::{std_into_error, ApiStdResponse},
    return_err, return_ok, AppState,
};

pub mod types {
    use poem_openapi::{
        payload::{Attachment, PlainText},
        types::multipart::Upload,
        ApiResponse, Multipart, Object,
    };
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Multipart)]
    pub struct UploadPayload {
        pub file: Upload,
    }

    #[derive(Object, Serialize, Default)]
    pub struct UploadFileRes {
        pub result: String,
    }

    #[derive(Debug, ApiResponse)]
    pub enum GetFileResponse {
        #[oai(status = 200)]
        Ok(Attachment<Vec<u8>>),
        /// File not found
        #[oai(status = 404)]
        NotFound,
        #[oai(status = 500)]
        InternalError(PlainText<String>),
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct ReadDirResp {
        pub current_dir: String,
        pub entry: Vec<DirEntry>,
    }

    #[derive(Object, Serialize, Default, Deserialize)]
    pub struct DirEntry {
        pub file_name: String,
        pub file_type: String,
        pub permissions: String,
        pub size: u64,
        pub user: String,
        pub group: String,
        pub modified: String,
    }

    #[derive(Debug, Multipart)]
    pub struct SftpUploadPayload {
        pub ip: String,
        pub file: Upload,
        pub namespace: String,
        pub file_path: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SftpUploadFileRes {
        pub result: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SftpRemovePayload {
        pub namespace: String,
        pub ip: String,
        /// delete type, dir or file
        pub remove_type: String,
        pub path: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SftpRemoveFileRes {
        pub result: String,
    }
}

macro_rules! unwrap_or_response {
    ($ret:expr) => {
        match $ret {
            Ok(v) => v,
            Err(e) => return types::GetFileResponse::InternalError(PlainText(e.to_string())),
        }
    };
}

pub struct FileApi;

#[OpenApi(prefix_path = "/file", tag = super::Tag::File)]
impl FileApi {
    #[oai(path = "/upload", method = "post")]
    async fn upload(
        &self,
        _state: Data<&AppState>,
        _session: &Session,
        upload: types::UploadPayload,
    ) -> Result<ApiStdResponse<types::UploadFileRes>> {
        let filename = upload.file.file_name().map(ToString::to_string);
        let data = upload.file.into_vec().await.map_err(std_into_error)?;

        create_dir_all("/tmp/jiascheduler")
            .await
            .map_err(std_into_error)?;

        let target_file = format!(
            "/tmp/jiascheduler/{}",
            filename.map_or("upload".to_string(), |v| v)
        );

        let mut tmp_file = File::create(target_file.clone())
            .await
            .map_err(std_into_error)?;

        tmp_file.write_all(&data).await.map_err(std_into_error)?;
        return_ok!(types::UploadFileRes {
            result: target_file
        })
    }

    #[oai(path = "/get/:filename", method = "get")]
    async fn get(&self, Path(filename): Path<String>) -> types::GetFileResponse {
        let buf = PathBuf::from(filename);
        let name = buf.file_name();

        let name = match name {
            Some(v) if !v.is_empty() => v.to_str().unwrap(),
            _ => return types::GetFileResponse::NotFound,
        };

        let target_path = format!("/tmp/jiascheduler/{}", name);

        let data = fs::read(target_path).await;

        let data = unwrap_or_response!(data);

        let mut attachment = Attachment::new(data).attachment_type(AttachmentType::Attachment);
        attachment = attachment.filename(name);
        types::GetFileResponse::Ok(attachment)
    }

    #[oai(path = "/sftp/download", method = "get")]
    async fn download(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Query(namespace): Query<String>,
        Query(file_path): Query<String>,
        Query(ip): Query<String>,
    ) -> types::GetFileResponse {
        let svc = state.service();
        let instance_record = unwrap_or_response!(
            svc.instance
                .get_one_user_server_with_permission(
                    state.clone(),
                    &user_info,
                    namespace,
                    ip.clone()
                )
                .await
        )
        .map_or(Err(anyhow!("not found")), |v| Ok(v));
        let instance_record = unwrap_or_response!(instance_record);
        let password =
            unwrap_or_response!(state.decrypt(instance_record.password.unwrap_or_default()));

        let ssh_session = unwrap_or_response!(
            SshSession::connect(ConnectParams {
                user: instance_record.sys_user.unwrap_or_default(),
                password,
                addrs: (ip, 22),
            })
            .await
        );

        let sftp_session = unwrap_or_response!(ssh_session.sftp_client().await);
        let data = unwrap_or_response!(sftp_session.read(&file_path).await);

        let name = std::path::Path::new(&file_path)
            .file_name()
            .map(|v| v.to_str())
            .flatten()
            .map_or("download.tmp".to_string(), |v| v.to_owned());

        let mut attachment = Attachment::new(data).attachment_type(AttachmentType::Attachment);
        attachment = attachment.filename(name);
        types::GetFileResponse::Ok(attachment)
    }

    #[oai(path = "/sftp/read-dir", method = "get")]
    async fn sftp_read_dir(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Query(namespace): Query<String>,
        Query(ip): Query<String>,
        Query(dir): Query<Option<String>>,
    ) -> Result<ApiStdResponse<types::ReadDirResp>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, namespace, ip.clone())
            .await?
            .map_or(Err(anyhow!("not found")), |v| Ok(v))?;
        let password = state.decrypt(instance_record.password.unwrap_or_default())?;
        let ssh_session = SshSession::connect(ConnectParams {
            user: instance_record.sys_user.unwrap_or_default(),
            password,
            addrs: (ip, 22),
        })
        .await?;

        let sft_session = ssh_session.sftp_client().await?;

        let current_dir = sft_session
            .canonicalize(dir.filter(|v| v != "").map_or("./".to_string(), |v| v))
            .await
            .map_err(std_into_error)?;

        let dir = sft_session
            .read_dir(&current_dir)
            .await
            .map_err(std_into_error)?;

        let mut resp = types::ReadDirResp {
            current_dir,
            entry: vec![],
        };

        for entry in dir {
            let meta = entry.metadata();
            let permissions = format!("{}", meta.permissions());
            let file_type = format!("{:?}", entry.file_type()).to_string();
            let file_name = entry.file_name();
            let modified = local_time!(DateTime::<Utc>::from(
                UNIX_EPOCH + Duration::from_secs(meta.mtime.unwrap_or(0) as u64),
            ));
            let user = if let Some(user) = &meta.user {
                user.to_string()
            } else {
                meta.uid.unwrap_or(0).to_string()
            };

            let group = if let Some(user) = &meta.group {
                user.to_string()
            } else {
                meta.gid.unwrap_or(0).to_string()
            };
            let size = meta.size.unwrap_or(0);

            resp.entry.push(types::DirEntry {
                file_name,
                file_type,
                permissions,
                modified,
                size,
                user,
                group,
            })
        }

        return_ok!(resp)
    }

    #[oai(path = "/sftp/upload", method = "post")]
    async fn sftp_upload(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,

        upload: types::SftpUploadPayload,
    ) -> Result<ApiStdResponse<types::SftpUploadFileRes>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(
                state.clone(),
                &user_info,
                upload.namespace,
                upload.ip.clone(),
            )
            .await?
            .map_or(Err(anyhow!("not found")), |v| Ok(v))?;
        let password = state.decrypt(instance_record.password.unwrap_or_default())?;
        let ssh_session = SshSession::connect(ConnectParams {
            user: instance_record.sys_user.unwrap_or_default(),
            password,
            addrs: (upload.ip, 22),
        })
        .await?;

        let dir = std::path::Path::new(&upload.file_path)
            .parent()
            .map(|v| v.to_str())
            .flatten();

        let sftp_session = ssh_session.sftp_client().await?;

        if let Some(dir) = dir {
            let is_exists = sftp_session.try_exists(dir).await.map_err(std_into_error)?;
            if !is_exists {
                sftp_session.create_dir(dir).await.map_err(std_into_error)?;
            }
        }

        let data = upload.file.into_vec().await.map_err(std_into_error)?;

        let mut file = sftp_session
            .create(upload.file_path)
            .await
            .map_err(std_into_error)?;

        file.write_all(&data).await.map_err(std_into_error)?;

        return_ok!(types::SftpUploadFileRes {
            result: "success".to_string()
        })
    }

    #[oai(path = "/sftp/remove", method = "post")]
    async fn sftp_remove(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SftpRemovePayload>,
    ) -> Result<ApiStdResponse<types::SftpRemoveFileRes>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(
                state.clone(),
                &user_info,
                req.namespace,
                req.ip.clone(),
            )
            .await?
            .map_or(Err(anyhow!("not found")), |v| Ok(v))?;
        let password = state.decrypt(instance_record.password.unwrap_or_default())?;
        let ssh_session = SshSession::connect(ConnectParams {
            user: instance_record.sys_user.unwrap_or_default(),
            password,
            addrs: (req.ip, 22),
        })
        .await?;

        let sftp_session = ssh_session.sftp_client().await?;

        if req.remove_type == "dir" {
            sftp_session
                .remove_dir(req.path)
                .await
                .map_err(std_into_error)?;
        } else {
            sftp_session
                .remove_file(req.path)
                .await
                .map_err(std_into_error)?;
        }

        return_ok!(types::SftpRemoveFileRes {
            result: "success".to_string()
        })
    }

    #[oai(path = "/sftp/tunnel/read-dir", method = "get")]
    async fn sftp_tunnel_read_dir(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Query(namespace): Query<String>,
        Query(ip): Query<String>,
        Query(dir): Query<Option<String>>,
    ) -> Result<ApiStdResponse<types::ReadDirResp>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(
                state.clone(),
                &user_info,
                namespace.clone(),
                ip.clone(),
            )
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let user = instance_record
            .sys_user
            .filter(|v| v != "")
            .ok_or(anyhow!("no sys user"))?;
        let password = instance_record
            .password
            .filter(|v| v != "")
            .ok_or(anyhow!("no password"))?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let password = state.decrypt(password)?;
        let ret = svc
            .ssh
            .sftp_read_dir(namespace, ip, port, dir, user, password)
            .await?;

        let dir_detail: types::ReadDirResp = serde_json::from_value(ret).map_err(std_into_error)?;

        return_ok!(dir_detail);
    }

    #[oai(path = "/sftp/tunnel/upload", method = "post")]
    async fn sftp_tunnel_upload(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        upload: types::SftpUploadPayload,
    ) -> Result<ApiStdResponse<types::SftpUploadFileRes>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(
                state.clone(),
                &user_info,
                upload.namespace.clone(),
                upload.ip.clone(),
            )
            .await?
            .ok_or(anyhow!("not found instance"))?;

        let user = instance_record
            .sys_user
            .filter(|v| v != "")
            .ok_or(anyhow!("no sys user"))?;
        let password = instance_record
            .password
            .filter(|v| v != "")
            .ok_or(anyhow!("no password"))?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let password = state.decrypt(password)?;

        let data = upload.file.into_vec().await.map_err(std_into_error)?;

        let ret = svc
            .ssh
            .sftp_upload(
                upload.namespace,
                instance_record.ip.clone(),
                port,
                user,
                password,
                upload.file_path,
                data,
            )
            .await?;

        return_ok!(types::SftpUploadFileRes { result: ret })
    }

    #[oai(path = "/sftp/tunnel/remove", method = "post")]
    async fn sftp_tunnel_remove(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SftpRemovePayload>,
    ) -> Result<ApiStdResponse<types::SftpRemoveFileRes>> {
        let v = vec!["file", "dir"];
        if !v.contains(&req.remove_type.as_str()) {
            return_err!("invalid remove type");
        }

        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(
                state.clone(),
                &user_info,
                req.namespace.clone(),
                req.ip.clone(),
            )
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let user = instance_record
            .sys_user
            .filter(|v| v != "")
            .ok_or(anyhow!("no sys user"))?;
        let password = instance_record
            .password
            .filter(|v| v != "")
            .ok_or(anyhow!("no password"))?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let password = state.decrypt(password)?;

        let ret = svc
            .ssh
            .sftp_remove(
                req.namespace,
                instance_record.ip.clone(),
                port,
                user,
                password,
                req.path,
                req.remove_type,
            )
            .await?;

        return_ok!(types::SftpRemoveFileRes { result: ret })
    }

    #[oai(path = "/sftp/tunnel/download", method = "get")]
    async fn sftp_tunnel_download(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Query(namespace): Query<String>,
        Query(file_path): Query<String>,
        Query(ip): Query<String>,
    ) -> types::GetFileResponse {
        let svc = state.service();
        let instance_record = unwrap_or_response!(
            svc.instance
                .get_one_user_server_with_permission(
                    state.clone(),
                    &user_info,
                    namespace.clone(),
                    ip.clone()
                )
                .await
        );

        let instance_record =
            unwrap_or_response!(instance_record.ok_or(anyhow!("not found instance")));

        let user = unwrap_or_response!(instance_record
            .sys_user
            .filter(|v| v != "")
            .ok_or(anyhow!("no sys user")));

        let password = unwrap_or_response!(instance_record
            .password
            .filter(|v| v != "")
            .ok_or(anyhow!("no password")));
        let port = unwrap_or_response!(instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port")));

        let password = unwrap_or_response!(state.decrypt(password));

        let data = unwrap_or_response!(
            svc.ssh
                .sftp_download(
                    namespace.clone(),
                    ip,
                    port,
                    user,
                    password,
                    file_path.clone()
                )
                .await
        );

        let name = std::path::Path::new(&file_path)
            .file_name()
            .map(|v| v.to_str())
            .flatten()
            .map_or("download.tmp".to_string(), |v| v.to_owned());

        let mut attachment = Attachment::new(data).attachment_type(AttachmentType::Attachment);
        attachment = attachment.filename(name);

        types::GetFileResponse::Ok(attachment)
    }
}
