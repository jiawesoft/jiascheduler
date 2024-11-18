use std::{path::PathBuf, sync::Arc};

use anyhow::Result;

use futures::SinkExt;
use futures_util::{
    stream::{SplitSink, SplitStream},
    StreamExt,
};

use poem::{
    handler,
    http::StatusCode,
    web::{
        websocket::{Message, WebSocket, WebSocketStream},
        Data,
        Json,
        Path,
        Query, // RemoteAddr,
    },
    FromRequest, IntoResponse, Request, RequestBody, Response, Result as PoemResult,
};

use serde::{Deserialize, Serialize};
use tokio::{
    fs::{self, create_dir_all, File},
    io::AsyncWriteExt,
    sync::RwLock,
};
use tracing::error;

use crate::{
    bridge::client::WsClient,
    comet::{
        types::{self, SshLoginParams},
        Comet,
    },
    return_response,
    scheduler::types::{SshConnectionOption, UploadFile},
};

pub mod middleware {
    use poem::{
        http::StatusCode,
        web::headers::{self, authorization::Bearer, HeaderMapExt},
        Endpoint, Error, Middleware, Request, Result,
    };

    pub fn bearer_auth(secret: &str) -> BearerAuth {
        BearerAuth {
            secret: secret.to_string(),
        }
    }

    pub struct BearerAuth {
        pub secret: String,
    }

    impl<E: Endpoint> Middleware<E> for BearerAuth {
        type Output = BasicAuthEndpoint<E>;

        fn transform(&self, ep: E) -> Self::Output {
            BasicAuthEndpoint {
                ep,
                secret: self.secret.clone(),
            }
        }
    }

    pub struct BasicAuthEndpoint<E> {
        ep: E,
        secret: String,
    }

    impl<E: Endpoint> Endpoint for BasicAuthEndpoint<E> {
        type Output = E::Output;
        async fn call(&self, req: Request) -> Result<Self::Output> {
            if let Some(auth) = req.headers().typed_get::<headers::Authorization<Bearer>>() {
                if auth.token() == self.secret {
                    return self.ep.call(req).await;
                }
            }
            Err(Error::from_status(StatusCode::UNAUTHORIZED))
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SecretHeader {
    pub assign_user: Option<(String, String)>,
    pub ssh_connection_params: Option<SshConnectionOption>,
}

// Implements a token extractor
impl<'a> FromRequest<'a> for SecretHeader {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> PoemResult<Self> {
        let header = req.headers();
        let username = header
            .get("X-Assign-Username")
            .and_then(|value| value.to_str().ok());

        let password = header
            .get("X-Assign-Password")
            .and_then(|value| value.to_str().ok());

        let ssh_user = header
            .get("X-Ssh-User")
            .and_then(|value| value.to_str().ok());
        let ssh_password = header
            .get("X-Ssh-Password")
            .and_then(|value| value.to_str().ok());
        let ssh_port = header.get("x-ssh-port").and_then(|value| {
            value
                .to_str()
                .ok()
                .map(|v| u16::from_str_radix(v, 10).ok())
                .flatten()
        });

        let mut assign = match (username, password) {
            (Some(u), Some(p)) => SecretHeader {
                assign_user: Some((u.to_string(), p.to_string())),
                ssh_connection_params: None,
            },
            _ => SecretHeader {
                assign_user: None,
                ssh_connection_params: None,
            },
        };

        if let (Some(u), Some(p), Some(port)) = (ssh_user, ssh_password, ssh_port) {
            assign.ssh_connection_params = Some(SshConnectionOption {
                user: u.to_string(),
                password: p.to_string(),
                port,
            });
        }

        Ok(assign)
    }
}

#[handler]
pub fn ws(
    ws: WebSocket,
    secret_header: SecretHeader,
    // _remote_addr: &RemoteAddr,
    Path(namespace): Path<String>,

    comet: Data<&Comet>,
) -> impl IntoResponse {
    let mut bridge = comet.bridge.clone();
    let mut comet = comet.clone();

    ws.on_upgrade(|socket| async move {
        let (mut sink, mut stream) = socket.split();

        let mut client: WsClient<
            SplitSink<WebSocketStream, Message>,
            SplitStream<WebSocketStream>,
        > = WsClient::new(Some(bridge.clone()));

        client.set_rw(sink, stream);

        if let Err(e) = client.auth(namespace, comet.secret.clone()).await {
            error!("failed to auth incoming connection - {e}");
            return;
        }

        client.start_processing_to_client_msg();

        let (namespace, agent_ip) = (client.get_namespace(), client.get_local_ip());

        comet
            .client_online(
                secret_header,
                client.get_is_initialized(),
                namespace.clone(),
                agent_ip.clone(),
                client.sender(),
            )
            .await;

        let ncomet = comet.clone();
        client
            .recv(|msg| async move { ncomet.handle(msg).await })
            .await;

        comet.client_offline(namespace, agent_ip).await;

        client.drop().await;
    })
}

const UPLOAD_DIR: &str = "/tmp/jiascheduler-comet";

#[allow(dead_code)]
async fn upload_file(file: Option<UploadFile>) -> Result<Option<UploadFile>> {
    if file.is_none() {
        return Ok(file);
    }
    let file = file.unwrap();
    if file.data.is_none() || file.filename == "" {
        return Ok(None);
    }
    create_dir_all(UPLOAD_DIR).await?;
    let target_file = format!("{}/{}", UPLOAD_DIR, file.filename);
    let mut tmp_file = File::create(target_file.clone()).await?;
    tmp_file.write_all(&file.data.unwrap()).await?;
    Ok(Some(UploadFile {
        filename: file.filename,
        data: None,
    }))
}

#[handler]
pub async fn get_file(Path(filename): Path<String>) -> impl IntoResponse {
    let buf = PathBuf::from(filename.clone());
    let name = buf.file_name();
    let resp = Response::builder();

    let name = match name {
        Some(v) if !v.is_empty() => v.to_str().unwrap(),
        _ => return resp.status(StatusCode::NOT_FOUND).body("not found"),
    };

    let target_path = format!("{}/{}", UPLOAD_DIR, name);

    let data = match fs::read(target_path).await {
        Ok(v) => v,
        Err(e) => {
            return resp
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(e.to_string())
        }
    };

    resp.header(
        "Content-Disposition",
        format!("attachment; filename=\"{}\"", name),
    )
    .content_type("application/octet-stream")
    .body(data)
}

#[handler]
pub async fn dispatch(
    comet: Data<&Comet>,
    Json(mut req): Json<types::DispatchJobRequest>,
) -> Json<serde_json::Value> {
    // let upload = req.dispatch_params.base_job.upload_file.take();

    // req.dispatch_params.base_job.upload_file = match upload_file(upload).await {
    //     Ok(v) => v,
    //     Err(e) => return_response!(code: 50000, e.to_string()),
    // };

    let ret = comet.dispatch(req).await;
    match ret {
        Ok(v) => {
            return_response!(json:v);
        }
        Err(e) => return_response!(code: 50000, e.to_string()),
    }
}

#[handler]
pub async fn runtime_action(
    comet: Data<&Comet>,
    Json(mut req): Json<types::RuntimeActionRequest>,
) -> Json<serde_json::Value> {
    let ret = comet.runtime_action(req).await;
    match ret {
        Ok(v) => {
            return_response!(json:v);
        }
        Err(e) => return_response!(code: 50000, e.to_string()),
    }
}

#[handler]
pub async fn ssh_register(
    webssh: WebSocket,
    // Path(namespace): Path<String>,
    Path(key): Path<String>,
    comet: Data<&Comet>,
) -> impl IntoResponse {
    // let mut bridge = comet.bridge.clone();
    let mut comet = comet.clone();
    webssh.on_upgrade(move |socket| async move {
        comet.register_ssh_stream(key, socket).await;
    })
}

#[handler]
pub async fn proxy_ssh(
    webssh: WebSocket,
    Path(_ip): Path<String>,
    Query(login_params): Query<SshLoginParams>,
    comet: Data<&Comet>,
) -> impl IntoResponse {
    let mut comet = comet.clone();

    webssh.on_upgrade(move |socket| async move {
        let (mut clientsink, mut clientstream) = socket.split();

        let target_stream = comet
            .get_ssh_stream(login_params)
            .await
            .expect("failed to get websocket stream");

        let (mut serversink, mut serverstream) = target_stream.split();

        let client_live = Arc::new(RwLock::new(true));
        let server_live = client_live.clone();

        // Relay client messages to the server we are proxying
        tokio::spawn(async move {
            while let Some(Ok(msg)) = clientstream.next().await {
                if let Err(_) = serversink.send(msg.into()).await {
                    break;
                }
                if !*client_live.read().await {
                    break;
                };
            }
            *client_live.write().await = false;
            let _ = serversink.close().await;
        });

        // Relay server messages to the client
        tokio::spawn(async move {
            while let Some(Ok(msg)) = serverstream.next().await {
                if let Err(_) = clientsink.send(msg.into()).await {
                    break;
                };

                if !*server_live.read().await {
                    break;
                };
            }
            *server_live.write().await = false;
            let _ = clientsink.close().await;
        });
    })
}

#[handler]
pub async fn sftp_read_dir(
    comet: Data<&Comet>,
    Json(mut req): Json<types::SftpReadDirRequest>,
) -> Json<serde_json::Value> {
    let ret = comet.sftp_read_dir(req).await;
    match ret {
        Ok(v) => {
            return_response!(json:v);
        }
        Err(e) => return_response!(code: 50000, e.to_string()),
    }
}

#[handler]
pub async fn sftp_upload(
    comet: Data<&Comet>,
    Json(mut req): Json<types::SftpUploadRequest>,
) -> Json<serde_json::Value> {
    let ret = comet.sftp_upload(req).await;
    match ret {
        Ok(v) => {
            return_response!(json:v);
        }
        Err(e) => return_response!(code: 50000, e.to_string()),
    }
}

#[handler]
pub async fn sftp_download(
    comet: Data<&Comet>,
    Json(mut req): Json<types::SftpDownloadRequest>,
) -> Json<serde_json::Value> {
    let ret = comet.sftp_download(req).await;
    match ret {
        Ok(v) => {
            return_response!(json:v);
        }
        Err(e) => return_response!(code: 50000, e.to_string()),
    }
}

#[handler]
pub async fn sftp_remove(
    comet: Data<&Comet>,
    Json(mut req): Json<types::SftpRemoveRequest>,
) -> Json<serde_json::Value> {
    let ret = comet.sftp_remove(req).await;
    match ret {
        Ok(v) => {
            return_response!(json:v);
        }
        Err(e) => return_response!(code: 50000, e.to_string()),
    }
}
