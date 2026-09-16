use std::{
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};

use anyhow::anyhow;

use chrono::{DateTime, Utc};
use poem::{
    handler,
    http::{header, HeaderValue},
    session::Session,
    web::{Data, Query as WebQuery},
    Body, Response, Result,
};
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
    error::NoPermission,
    local_time,
    logic::{
        self,
        ssh::{ConnectParams2, Session as SshSession},
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
        #[oai(status = 403)]
        NotAllow,
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
        pub instance_id: String,
        pub file: Upload,
        pub namespace: String,
        pub file_path: String,
        /// Login user selected from the instance `sys_users` list. Empty means
        /// the default login user of the instance.
        #[oai(default)]
        pub sys_user: Option<String>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SftpUploadFileRes {
        pub result: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SftpRemovePayload {
        pub instance_id: String,
        /// delete type, dir or file
        pub remove_type: String,
        pub path: String,
        /// Login user selected from the instance `sys_users` list. Empty means
        /// the default login user of the instance.
        #[oai(default)]
        pub sys_user: Option<String>,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SftpRemoveFileRes {
        pub result: String,
    }

    /// Chunked upload payload for one chunk. `data` is base64 encoded, which
    /// only inflates it 1.33x and stays far below the frame limit.
    #[derive(Object, Serialize, Deserialize, Default)]
    pub struct SftpUploadChunkPayload {
        pub instance_id: String,
        pub file_path: String,
        pub session_id: String,
        pub offset: u64,
        pub data: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SftpChunkRes {
        pub result: String,
        /// Offset already written to / read from the remote file.
        pub next_offset: u64,
        /// Download only: the reused session id.
        #[oai(default)]
        pub session_id: Option<String>,
    }

    #[derive(Object, Serialize, Deserialize, Default)]
    pub struct SftpDownloadFinishPayload {
        pub instance_id: String,
        pub session_id: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct SftpChunkStartRes {
        pub session_id: String,
        /// Suggested chunk size.
        pub chunk_size: u64,
    }

    #[derive(Object, Serialize, Deserialize, Default)]
    pub struct SftpFinishPayload {
        pub instance_id: String,
        pub file_path: String,
        pub session_id: String,
        pub total_size: u64,
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

/// Parse the `sys_user` field of a multipart upload form.
fn multipart_sys_user(req: &types::SftpUploadPayload) -> Option<String> {
    req.sys_user.clone().filter(|v| v.trim() != "")
}

#[OpenApi(prefix_path = "/file", tag = super::Tag::File)]
impl FileApi {
    #[oai(path = "/upload", method = "post")]
    async fn upload(
        &self,
        state: Data<&AppState>,
        _session: &Session,
        upload: types::UploadPayload,
        user_info: Data<&logic::types::UserInfo>,
    ) -> Result<ApiStdResponse<types::UploadFileRes>> {
        if !state.can_upload_file(&user_info.user_id).await? {
            return Err(NoPermission().into());
        }
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
    async fn get(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Path(filename): Path<String>,
    ) -> types::GetFileResponse {
        if !unwrap_or_response!(state.can_upload_file(&user_info.user_id).await) {
            return types::GetFileResponse::NotAllow;
        }

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
        Query(instance_id): Query<String>,
        Query(file_path): Query<String>,
        Query(sys_user): Query<Option<String>>,
    ) -> types::GetFileResponse {
        let svc = state.service();
        let instance_record = unwrap_or_response!(
            svc.instance
                .get_one_user_server_with_permission(state.clone(), &user_info, instance_id)
                .await
        )
        .map_or(Err(anyhow!("not found")), |v| Ok(v));
        let instance_record = unwrap_or_response!(instance_record);
        let (user, auth_data) = unwrap_or_response!(super::instance::resolve_user_auth(
            &state,
            &instance_record,
            sys_user.as_deref()
        ));

        let ssh_session = unwrap_or_response!(
            SshSession::connect2(ConnectParams2 {
                user,
                auth: auth_data,
                addrs: (instance_record.ip, 22),
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
        Query(instance_id): Query<String>,
        Query(dir): Query<Option<String>>,
        Query(sys_user): Query<Option<String>>,
    ) -> Result<ApiStdResponse<types::ReadDirResp>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, instance_id.clone())
            .await?
            .map_or(Err(anyhow!("not found")), |v| Ok(v))?;
        let (user, auth_data) = super::instance::resolve_user_auth(
            &state,
            &instance_record,
            sys_user.as_deref(),
        )?;
        let ssh_session = SshSession::connect2(ConnectParams2 {
            user,
            auth: auth_data,
            addrs: (instance_record.ip, 22),
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
        req: types::SftpUploadPayload,
    ) -> Result<ApiStdResponse<types::SftpUploadFileRes>> {
        let svc = state.service();
        let sys_user = multipart_sys_user(&req);
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id)
            .await?
            .map_or(Err(anyhow!("not found")), |v| Ok(v))?;
        let (user, auth_data) =
            super::instance::resolve_user_auth(&state, &instance_record, sys_user.as_deref())?;
        let ssh_session = SshSession::connect2(ConnectParams2 {
            user,
            auth: auth_data,
            addrs: (instance_record.ip, 22),
        })
        .await?;

        let dir = std::path::Path::new(&req.file_path)
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

        let data = req.file.into_vec().await.map_err(std_into_error)?;

        let mut file = sftp_session
            .create(req.file_path)
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
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id)
            .await?
            .map_or(Err(anyhow!("not found")), |v| Ok(v))?;
        let (user, auth_data) = super::instance::resolve_user_auth(
            &state,
            &instance_record,
            req.sys_user.as_deref(),
        )?;
        let ssh_session = SshSession::connect2(ConnectParams2 {
            user,
            auth: auth_data,
            addrs: (instance_record.ip, 22),
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
        Query(instance_id): Query<String>,
        Query(dir): Query<Option<String>>,
        Query(sys_user): Query<Option<String>>,
    ) -> Result<ApiStdResponse<types::ReadDirResp>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let (user, auth_data) = super::instance::resolve_user_auth(
            &state,
            &instance_record,
            sys_user.as_deref(),
        )?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let ret = svc
            .ssh
            .sftp_read_dir(
                instance_record.namespace,
                instance_record.ip,
                instance_record.mac_addr,
                port,
                dir,
                user,
                auth_data,
            )
            .await?;

        let dir_detail: types::ReadDirResp = serde_json::from_value(ret).map_err(std_into_error)?;

        return_ok!(dir_detail);
    }

    /// Whole-file upload, kept for internal and compatibility use; the web ui
    /// uses the chunked endpoints.
    #[oai(path = "/sftp/tunnel/upload", method = "post")]
    async fn sftp_tunnel_upload(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        req: types::SftpUploadPayload,
    ) -> Result<ApiStdResponse<types::SftpUploadFileRes>> {
        let svc = state.service();
        let sys_user = multipart_sys_user(&req);
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;

        let (user, auth_data) =
            super::instance::resolve_user_auth(&state, &instance_record, sys_user.as_deref())?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let data = req.file.into_vec().await.map_err(std_into_error)?;

        let ret = svc
            .ssh
            .sftp_upload(
                req.namespace,
                instance_record.ip,
                instance_record.mac_addr,
                port,
                user,
                auth_data,
                req.file_path,
                data,
            )
            .await?;

        return_ok!(types::SftpUploadFileRes { result: ret })
    }

    /// Chunked upload: start a session.
    #[oai(path = "/sftp/tunnel/upload/start", method = "post")]
    async fn sftp_upload_start(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SftpFinishPayload>,
    ) -> Result<ApiStdResponse<types::SftpChunkStartRes>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let (user, auth_data) = super::instance::resolve_user_auth(&state, &instance_record, None)?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let chunk_size = svc
            .ssh
            .sftp_upload_start(
                instance_record.namespace,
                instance_record.ip,
                instance_record.mac_addr,
                port,
                user,
                auth_data,
                req.file_path,
                req.total_size,
                req.session_id.clone(),
            )
            .await?;

        return_ok!(types::SftpChunkStartRes {
            session_id: req.session_id,
            chunk_size: if chunk_size == 0 {
                logic::ssh::SFTP_CHUNK_SIZE as u64
            } else {
                chunk_size
            },
        });
    }

    /// Chunked upload: write one chunk.
    #[oai(path = "/sftp/tunnel/upload/chunk", method = "post")]
    async fn sftp_upload_chunk(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SftpUploadChunkPayload>,
    ) -> Result<ApiStdResponse<types::SftpChunkRes>> {
        use rustc_serialize::base64::FromBase64;

        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let (user, auth_data) = super::instance::resolve_user_auth(&state, &instance_record, None)?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let comet_addr = svc
            .ssh
            .get_comet_addr(&instance_record.ip, &instance_record.mac_addr)
            .await?;

        let data = req.data.as_bytes().from_base64().map_err(std_into_error)?;

        let next_offset = svc
            .ssh
            .sftp_upload_chunk(
                instance_record.namespace,
                comet_addr,
                instance_record.mac_addr,
                automate::bridge::msg::SftpUploadChunkParams {
                    session_id: req.session_id,
                    seq: req.offset / (logic::ssh::SFTP_CHUNK_SIZE as u64),
                    offset: req.offset,
                    data,
                    ip: instance_record.ip,
                    port,
                    user,
                    auth_data,
                    filepath: req.file_path,
                },
            )
            .await?;

        return_ok!(types::SftpChunkRes {
            result: "success".to_string(),
            next_offset,
            session_id: None,
        });
    }

    /// Chunked upload: finish and verify the size.
    #[oai(path = "/sftp/tunnel/upload/finish", method = "post")]
    async fn sftp_upload_finish(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SftpFinishPayload>,
    ) -> Result<ApiStdResponse<types::SftpUploadFileRes>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let (user, auth_data) = super::instance::resolve_user_auth(&state, &instance_record, None)?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let comet_addr = svc
            .ssh
            .get_comet_addr(&instance_record.ip, &instance_record.mac_addr)
            .await?;

        let data = svc
            .ssh
            .sftp_upload_finish(
                instance_record.namespace,
                comet_addr,
                instance_record.mac_addr,
                automate::bridge::msg::SftpUploadFinishParams {
                    session_id: req.session_id,
                    total_size: req.total_size,
                    ip: instance_record.ip,
                    port,
                    user,
                    auth_data,
                    filepath: req.file_path,
                },
            )
            .await?;

        // Return the agent result (the actual remote size) so the client and
        // operators can confirm the transfer.
        return_ok!(types::SftpUploadFileRes {
            result: data.to_string()
        })
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
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let (user, auth_data) = super::instance::resolve_user_auth(
            &state,
            &instance_record,
            req.sys_user.as_deref(),
        )?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let ret = svc
            .ssh
            .sftp_remove(
                instance_record.namespace,
                instance_record.ip,
                instance_record.mac_addr,
                port,
                user,
                auth_data,
                req.path,
                req.remove_type,
            )
            .await?;

        return_ok!(types::SftpRemoveFileRes { result: ret })
    }

    /// Chunked download: query the remote file size.
    #[oai(path = "/sftp/tunnel/download/stat", method = "get")]
    async fn sftp_download_stat(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Query(file_path): Query<String>,
        Query(instance_id): Query<String>,
        Query(sys_user): Query<Option<String>>,
    ) -> Result<ApiStdResponse<types::SftpChunkRes>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let (user, auth_data) =
            super::instance::resolve_user_auth(&state, &instance_record, sys_user.as_deref())?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        // Downloads reuse a session as well: the client pulls chunks with the
        // returned session_id.
        let session_id = format!("dl-{}", nanoid::nanoid!(16));
        let (size, _chunk) = svc
            .ssh
            .sftp_download_stat(
                instance_record.namespace,
                instance_record.ip,
                instance_record.mac_addr,
                port,
                user,
                auth_data,
                file_path,
                session_id.clone(),
            )
            .await?;

        return_ok!(types::SftpChunkRes {
            result: size.to_string(),
            next_offset: size,
            session_id: Some(session_id),
        });
    }

    /// Chunked download: fetch one chunk, returned as base64.
    #[oai(path = "/sftp/tunnel/download/chunk", method = "get")]
    async fn sftp_download_chunk(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Query(file_path): Query<String>,
        Query(instance_id): Query<String>,
        Query(offset): Query<u64>,
        Query(len): Query<u32>,
        Query(session_id): Query<Option<String>>,
        Query(sys_user): Query<Option<String>>,
    ) -> Result<ApiStdResponse<types::SftpChunkRes>> {
        use rustc_serialize::base64::ToBase64 as _;

        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;
        let (user, auth_data) =
            super::instance::resolve_user_auth(&state, &instance_record, sys_user.as_deref())?;
        let port = instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port"))?;

        let comet_addr = svc
            .ssh
            .get_comet_addr(&instance_record.ip, &instance_record.mac_addr)
            .await?;

        let data = svc
            .ssh
            .sftp_download_chunk(
                instance_record.namespace,
                comet_addr,
                instance_record.mac_addr,
                automate::bridge::msg::SftpDownloadChunkParams {
                    session_id: session_id.unwrap_or_default(),
                    ip: instance_record.ip,
                    port,
                    user,
                    auth_data,
                    filepath: file_path,
                    offset,
                    len,
                },
            )
            .await?;

        return_ok!(types::SftpChunkRes {
            result: data.to_base64(rustc_serialize::base64::STANDARD),
            next_offset: offset + data.len() as u64,
            session_id: None,
        });
    }

    /// Chunked download: end the session and release the agent side connection.
    #[oai(path = "/sftp/tunnel/download/finish", method = "post")]
    async fn sftp_download_finish(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Json(req): Json<types::SftpDownloadFinishPayload>,
    ) -> Result<ApiStdResponse<types::SftpRemoveFileRes>> {
        let svc = state.service();
        let instance_record = svc
            .instance
            .get_one_user_server_with_permission(state.clone(), &user_info, req.instance_id)
            .await?
            .ok_or(anyhow!("not found instance"))?;

        let comet_addr = svc
            .ssh
            .get_comet_addr(&instance_record.ip, &instance_record.mac_addr)
            .await?;

        svc.ssh
            .sftp_download_finish(
                instance_record.namespace,
                comet_addr,
                instance_record.mac_addr.clone(),
                automate::bridge::msg::SftpDownloadFinishParams {
                    session_id: req.session_id,
                },
                instance_record.ip,
            )
            .await?;

        return_ok!(types::SftpRemoveFileRes {
            result: "success".to_string()
        })
    }

    /// Whole-file download, kept for compatibility; the web ui uses the chunked
    /// download so that large files are supported.
    #[oai(path = "/sftp/tunnel/download", method = "get")]
    async fn sftp_tunnel_download(
        &self,
        state: Data<&AppState>,
        user_info: Data<&logic::types::UserInfo>,
        Query(file_path): Query<String>,
        Query(instance_id): Query<String>,
        Query(sys_user): Query<Option<String>>,
    ) -> types::GetFileResponse {
        let svc = state.service();
        let instance_record = unwrap_or_response!(
            svc.instance
                .get_one_user_server_with_permission(state.clone(), &user_info, instance_id)
                .await
        );

        let instance_record =
            unwrap_or_response!(instance_record.ok_or(anyhow!("not found instance")));

        let (user, auth_data) = unwrap_or_response!(super::instance::resolve_user_auth(
            &state,
            &instance_record,
            sys_user.as_deref()
        ));

        let port = unwrap_or_response!(instance_record
            .ssh_port
            .filter(|&v| v != 0)
            .ok_or(anyhow!("no ssh port")));

        let data = unwrap_or_response!(
            svc.ssh
                .sftp_download(
                    instance_record.namespace,
                    instance_record.ip,
                    instance_record.mac_addr,
                    port,
                    user,
                    auth_data,
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

#[derive(serde::Deserialize)]
pub struct DownloadStreamQuery {
    pub file_path: String,
    pub instance_id: String,
    #[serde(default)]
    pub sys_user: Option<String>,
}

/// Stream a remote file straight to the browser.
///
/// This is deliberately a plain poem handler instead of an `#[oai]` endpoint:
/// OpenAPI endpoints have to produce a value, while streaming needs to write the
/// response body incrementally. It mirrors how `proxy_webssh` is built and
/// registered, and it is protected by the same `AuthMiddleware`.
#[handler]
pub async fn download_stream(
    state: Data<&AppState>,
    user_info: Data<&logic::types::UserInfo>,
    WebQuery(query): WebQuery<DownloadStreamQuery>,
) -> Result<Response> {
    // `#[handler]` produces Result<Response>, so this stays a plain route.
    let svc = state.service();
    let instance_record = svc
        .instance
        .get_one_user_server_with_permission(state.clone(), &user_info, query.instance_id)
        .await?
        .ok_or(anyhow!("not found instance"))?;

    let (user, auth_data) = super::instance::resolve_user_auth(
        &state,
        &instance_record,
        query.sys_user.as_deref(),
    )?;

    let port = instance_record
        .ssh_port
        .filter(|&v| v != 0)
        .ok_or(anyhow!("no ssh port"))?;

    // The remote size is resolved inside the stream (it opens the agent session
    // there), so no Content-Length is sent and the browser uses chunked transfer
    // encoding. Feeding the body from the live connection is what keeps both the
    // console and the browser from buffering the whole file.
    let stream = svc.ssh.sftp_download_stream(
        instance_record.namespace.clone(),
        instance_record.ip.clone(),
        instance_record.mac_addr.clone(),
        port,
        user,
        auth_data,
        query.file_path.clone(),
    );

    let name = query
        .file_path
        .rsplit('/')
        .next()
        .filter(|v| !v.is_empty())
        .unwrap_or("download.bin")
        .replace('"', "");

    // Only ascii safe characters are kept, so the header cannot be broken by the
    // file name. Non ascii names still download correctly, just without the
    // original name in the save dialog.
    let safe_name: String = name
        .chars()
        .map(|c| if c.is_ascii_graphic() && c != '"' { c } else { '_' })
        .collect();
    let safe_name = if safe_name.is_empty() {
        "download.bin".to_string()
    } else {
        safe_name
    };

    let mut response = Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{safe_name}\""),
        )
        .body(Body::from_bytes_stream(stream));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
