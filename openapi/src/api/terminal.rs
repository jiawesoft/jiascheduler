use std::sync::Arc;

use crate::logic::ssh::{ConnectParams, Session};
use crate::state::AppState;
use crate::{logic, return_err_to_wsconn};

use automate::Logic;
use automate::scheduler::types::SshConnectOption;
use automate::ssh::AuthData;
use futures::{SinkExt, StreamExt};

use poem::http::HeaderMap;
use poem::session::Session as WebSession;
use poem::web::websocket::WebSocket;
use poem::web::{Data, Path, Query};
use poem::{FromRequest, IntoResponse, Request, handler};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;

use tracing::{debug, error};
use url::Url;
use utils::json_into;

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
        /// user_source: manual | agent | sys_user
        #[serde(default)]
        pub user_source: Option<String>,
        /// Optional manual ssh username. When provided, manual account
        /// (user + auth) is preferred over the stored accounts.
        #[serde(default)]
        pub auth_type: Option<String>,
        /// Optional manual ssh password
        #[serde(default)]
        pub password: Option<String>,
        /// Optional manual ssh key_content
        #[serde(default)]
        pub key_content: Option<String>,
        #[serde(default)]
        pub port: Option<u16>,
        #[serde(default)]
        pub sys_user: Option<String>,
    }
}

/// Resolve which ssh account should be used to reach the instance.
fn resolve_account(
    inst: &crate::logic::types::UserServer,
    query: &types::WebSshQuery,
    decrypt: impl Fn(String) -> anyhow::Result<String>,
) -> anyhow::Result<SshConnectOption> {
    let port = super::instance::pick_ssh_port(inst, query.port);

    // manual account
    if query.user_source.as_ref().is_some_and(|v| v == "manual") {
        let user = query
            .sys_user
            .as_ref()
            .ok_or(anyhow::anyhow!("sys_user is required"))?;

        let auth_data = match query.auth_type.as_deref() {
            Some("key_content") => {
                AuthData::KeyContent(query.key_content.clone().unwrap_or_default())
            }
            _ => AuthData::Password(query.password.clone().unwrap_or_default()),
        };
        return Ok(SshConnectOption {
            user: user.clone(),
            port,
            auth_data,
        });
    }

    if query.user_source.is_none() || query.user_source.as_ref().is_some_and(|v| v != "agent") {
        let selected = query
            .sys_user
            .clone()
            .filter(|v| v.trim() != "")
            .or_else(|| inst.sys_user.clone().filter(|v| v.trim() != ""));

        // a login user configured on the instance
        if let Some(selected) = selected {
            if let Some((user, auth_data)) =
                super::instance::resolve_user_auth_of(&inst.sys_users, &selected, decrypt)?
            {
                return Ok(SshConnectOption {
                    user,
                    port,
                    auth_data,
                });
            }
        }
    }

    // the account reported by the agent
    let Some(ref register_data) = inst.register_data else {
        anyhow::bail!(
            "Notice: agent has not reported ssh connection options, please specify the account manually"
        );
    };

    let Some(user) = register_data.ssh_user.clone() else {
        anyhow::bail!("Notice: please set the system user first");
    };

    let Some(ref auth_data) = register_data.auth_data else {
        anyhow::bail!(
            "Notice: agent has not reported ssh auth data, please specify the account manually"
        );
    };

    Ok(SshConnectOption {
        user,
        port,
        auth_data: json_into::<_, AuthData>(auth_data).unwrap(),
    })
}

/// Webssh is deprecated.
///
/// This endpoint establishes a direct SSH connection to the target instance
/// using credentials stored in the database.
///
/// **Deprecated**: This method connects to the instance's SSH port directly,
/// which may fail when the instance is behind NAT or a firewall. Use
/// [`proxy_webssh`] instead, which routes the SSH traffic through the Comet
/// relay service (`/ssh/tunnel`) via the instance's registration pair.
///
/// The endpoint will be removed in a future release.
#[handler]
pub async fn webssh(
    Path(instance_id): Path<String>,
    state: Data<&AppState>,
    _session: &WebSession,
    user_info: Data<&logic::types::UserInfo>,
    Query(types::WebSshQuery { rows, cols, .. }): Query<types::WebSshQuery>,
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
    Query(query): Query<types::WebSshQuery>,
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

        let mut u = Url::parse(format!("ws://{}/ssh/tunnel", pair.1.comet_addr).as_ref()).unwrap();

        // Resolve the ssh account: explicit client account first, then the
        // login user configured on the instance, then the agent-reported one.
        let decrypt_state = state_clone.clone();
        let connect_opts = match resolve_account(&instance_record, &query, |v| {
            super::instance::decrypt_secret(&decrypt_state, v)
        }) {
            Ok(v) => v,
            Err(e) => {
                return_err_to_wsconn!(clientsink, format!("{e}"));
            }
        };

        u.query_pairs_mut()
            .append_pair("cols", &query.cols.to_string())
            .append_pair("rows", &query.rows.to_string())
            .append_pair("ip", &instance_record.ip)
            .append_pair("namespace", &instance_record.namespace)
            .append_pair("mac_addr", &instance_record.mac_addr)
            .append_pair(
                "connect_options",
                serde_json::to_string(&connect_opts).unwrap().as_ref(),
            );

        let mut ws_request = http::Request::builder()
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {}", comet_secret),
            )
            .uri(u.as_str());

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
