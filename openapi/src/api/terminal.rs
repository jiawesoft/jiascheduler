use std::sync::Arc;

use crate::logic::ssh::{ConnectParams, Session};
use crate::state::AppState;
use crate::{logic, return_err_to_wsconn};

use automate::Logic;
use futures::{SinkExt, StreamExt};
use poem::http::HeaderMap;
use poem::session::Session as WebSession;
use poem::web::websocket::WebSocket;
use poem::web::{Data, Path, Query};
use poem::{handler, FromRequest, IntoResponse, Request};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;

use tracing::{debug, error};

pub mod types {
    use serde::{Deserialize, Serialize};
    use serde_repr::*;

    #[derive(Debug, Deserialize_repr, Serialize_repr)]
    #[repr(u8)]
    pub enum MsgType {
        Resize = 1,
        Data = 2,
        Ping = 3,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Msg {
        pub r#type: MsgType,
        #[serde(default)]
        pub msg: String,
        #[serde(default)]
        pub cols: u32,
        #[serde(default)]
        pub rows: u32,
    }

    #[derive(Deserialize)]
    pub struct WebSshQuery {
        pub cols: u32,
        pub rows: u32,
    }
}

#[handler]
pub async fn webssh(
    Path(instance_id): Path<String>,
    state: Data<&AppState>,
    _session: &WebSession,
    user_info: Data<&logic::types::UserInfo>,
    Query(types::WebSshQuery { rows, cols }): Query<types::WebSshQuery>,
    ws: WebSocket,
) -> impl IntoResponse {
    let state_clone = state.clone();
    let user_id = user_info.user_id.clone();

    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();

        let svc = state_clone.service();

        let can_manage_instance = match state_clone.can_manage_instance(&user_id).await {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(sink, format!("Notice: failed to valid permissions, {e}"));
            }
        };

        let instance_record = if can_manage_instance {
            svc.instance
                .get_one_admin_server(None, None, Some(instance_id))
                .await
        } else {
            svc.instance
                .get_one_user_server(None, None, Some(instance_id), user_id.clone())
                .await
        };

        let instance_record = match instance_record {
            Ok(Some(v)) => v,
            Ok(None) => {
                return_err_to_wsconn!(sink, "Notice: invalid instance");
            }
            Err(e) => {
                return_err_to_wsconn!(sink, format!("Notice: failed get instance, {e}"));
            }
        };

        let password = match state_clone.decrypt(instance_record.password.unwrap_or_default()) {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(
                    sink,
                    format!("Notice: failed decrypt instance password, {e}")
                );
            }
        };

        let mut ssh = match Session::connect(ConnectParams {
            user: instance_record.sys_user.unwrap_or_default(),
            password,
            addrs: (instance_record.ip, instance_record.ssh_port.unwrap_or(22)),
        })
        .await
        {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(
                    sink,
                    format!("Notice: failed connect to target server, {e}")
                );
            }
        };

        let code = match ssh.call("bash", cols, rows, &mut sink, stream).await {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(sink, format!("Notice: connection closed, {e}"));
            }
        };

        debug!("web ssh exit code {code}");

        if let Err(e) = ssh.close().await {
            error!("failed close - {e}");
        }
    })
}

#[handler]
pub async fn proxy_webssh(
    req: &Request,
    headers: &HeaderMap,
    state: Data<&AppState>,
    Path(instance_id): Path<String>,
    user_info: Data<&logic::types::UserInfo>,
    Query(types::WebSshQuery { rows, cols }): Query<types::WebSshQuery>,
) -> impl IntoResponse {
    let state_clone = state.clone();
    let user_id = user_info.user_id.clone();

    let ws = WebSocket::from_request_without_body(req)
        .await
        .expect("failed parse request");

    let headers = headers.to_owned();
    let comet_secret = state.conf.comet_secret.clone();

    ws.on_upgrade(move |socket| async move {
        let (mut clientsink, mut clientstream) = socket.split();

        let svc = state_clone.service();

        let can_manage_instance = match state_clone.can_manage_instance(&user_id).await {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(
                    clientsink,
                    format!("Notice: failed to valid permissions, {e}")
                );
            }
        };

        let instance_record = if can_manage_instance {
            svc.instance
                .get_one_admin_server(None, None, Some(instance_id))
                .await
        } else {
            svc.instance
                .get_one_user_server(None, None, Some(instance_id), user_id.clone())
                .await
        };

        let instance_record = match instance_record {
            Ok(Some(v)) => v,
            Ok(None) => {
                return_err_to_wsconn!(clientsink, "Notice: no instance found");
            }
            Err(e) => {
                return_err_to_wsconn!(clientsink, format!("Notice: failed get instance, {e}"));
            }
        };

        let pair = match Logic::new(state_clone.redis().clone())
            .get_link_pair(&instance_record.ip, &instance_record.mac_addr)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(
                    clientsink,
                    format!("Notice: failed to get instance register info, {e}")
                );
            }
        };

        let Some(password_raw) = instance_record.password else {
            return_err_to_wsconn!(
                clientsink,
                format!("Notice: please set the instance password first")
            );
        };

        let password = match state_clone.decrypt(password_raw) {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(
                    clientsink,
                    format!("Notice: failed decrypt instance password, {e}")
                );
            }
        };

        let Some(user) = instance_record.sys_user  else {
            return_err_to_wsconn!(clientsink, "Notice: please set the system user first");
        };

        let Some(port) =  instance_record.ssh_port else {
            return_err_to_wsconn!(clientsink, "Notice: please set the ssh port first");
        };

        let uri = format!(
            "ws://{}/ssh/tunnel?cols={}&rows={}&user={}&password={}&ip={}&port={}&namespace={}&mac_addr={}",
            pair.1.comet_addr,
            cols,
            rows,
            user,
            password,
            instance_record.ip,
            port,
            instance_record.namespace,
            instance_record.mac_addr,
        );

        let mut ws_request = http::Request::builder()
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {}", comet_secret),
            )
            .uri(&uri);

        for (key, value) in headers.iter() {
            ws_request = ws_request.header(key, value);
        }

        // Start connection to server
        let (serversocket, _) = match connect_async(ws_request.body(()).unwrap()).await {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(
                    clientsink,
                    format!("Notice: failed connect to target instance, {e}")
                );
            }
        };
        let (mut serversink, mut serverstream) = serversocket.split();
        let client_live = Arc::new(RwLock::new(true));
        let server_live = client_live.clone();

        // Relay client messages to the server we are proxying
        tokio::spawn(async move {
            while let Some(ret) = clientstream.next().await {
                match ret {
                    Ok(msg) => {
                        if let poem::web::websocket::Message::Close(_) = msg {
                            break;
                        }
                        if let Err(_) = serversink.send(msg.into()).await {
                            break;
                        }
                        if !*client_live.read().await {
                            break;
                        };
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
            *client_live.write().await = false;
            let _ = serversink.close().await;
        });

        // Relay server messages to the client
        tokio::spawn(async move {
            while let Some(ret) = serverstream.next().await {
                match ret {
                    Ok(msg) => {
                        if let Err(_) = clientsink.send(msg.into()).await {
                            break;
                        };

                        if !*server_live.read().await {
                            break;
                        };
                    }
                    Err(_) => break,
                }
            }
            *server_live.write().await = false;
            let _ = clientsink.close().await;
        });
    })
}
