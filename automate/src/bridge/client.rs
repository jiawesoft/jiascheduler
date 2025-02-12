use std::{
    net::IpAddr,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use futures_util::{
    stream::{SplitSink, SplitStream},
    Future, SinkExt, StreamExt,
};

use moka::future::Cache;
use poem::web::websocket::{Message as PMessage, WebSocketStream as PWebSocketStream};

use serde_json::{json, Value};
use tokio::{
    net::TcpStream,
    sync::mpsc::{self, Receiver, Sender},
    time::timeout,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{ClientRequestBuilder, Message},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{error, info};

use crate::{
    get_endpoint,
    scheduler::types::{AssignUserOption, SshConnectionOption},
};

use super::{
    msg::{AuthParams, Msg, MsgKind, MsgReqKind, MsgState, TransactionMsg},
    protocol::Protocol,
    Bridge,
};

pub struct WsClient<W, R> {
    sender: Sender<(Msg, Option<Sender<MsgState>>)>,
    ws_writer: Option<W>,
    ws_reader: Option<R>,
    comet_secret: Option<String>,
    mac_addr: Option<String>,
    local_ip: Option<IpAddr>,
    namespace: Option<String>,
    is_initialized: Option<bool>,
    ssh_connection_option: Option<SshConnectionOption>,
    assign_user_option: Option<AssignUserOption>,
    msg_box: Cache<u64, TransactionMsg>,
    bridge: Option<Bridge>,
    receiver: Option<Receiver<(Msg, Option<Sender<MsgState>>)>>,
}

impl<W, R> WsClient<W, R> {
    pub fn new(bridge: Option<Bridge>) -> Self {
        let (sender, receiver) = mpsc::channel::<(Msg, Option<Sender<MsgState>>)>(100);
        let cache: Cache<u64, TransactionMsg> = Cache::builder()
            .time_to_live(Duration::from_secs(5))
            .build();

        Self {
            sender,
            bridge,
            local_ip: None,
            namespace: None,
            mac_addr: None,
            comet_secret: None,
            is_initialized: None,
            msg_box: cache,
            assign_user_option: None,
            ssh_connection_option: None,
            ws_writer: None,
            ws_reader: None,
            receiver: Some(receiver),
        }
    }

    pub fn set_namespace(&mut self, namespace: String) -> &mut Self {
        self.namespace = Some(namespace);
        self
    }

    pub fn set_local_ip(&mut self, local_ip: IpAddr) -> &mut Self {
        self.local_ip = Some(local_ip);
        self
    }

    pub fn set_comet_secret(&mut self, comet_secret: String) -> &mut Self {
        self.comet_secret = Some(comet_secret);
        self
    }

    pub fn set_mac_address(&mut self, mac_addr: String) -> &mut Self {
        self.mac_addr = Some(mac_addr);
        self
    }

    pub fn set_assign_user(&mut self, assign_user: AssignUserOption) -> &mut Self {
        self.assign_user_option = Some(assign_user);
        self
    }

    pub fn set_ssh_connection(&mut self, ssh_option: SshConnectionOption) -> &mut Self {
        self.ssh_connection_option = Some(ssh_option);
        self
    }

    pub fn sender(&self) -> Sender<(Msg, Option<Sender<MsgState>>)> {
        self.sender.clone()
    }

    pub async fn send_msg(&mut self, msg: Msg) -> Result<Option<Value>> {
        let (tx, mut rx) = mpsc::channel::<MsgState>(1);
        self.sender.send((msg, Some(tx.clone()))).await?;

        let resp = timeout(Duration::from_secs(10), rx.recv()).await?;

        if let Some(val) = resp {
            match val {
                MsgState::Completed(v) => return Ok(Some(v)),
                MsgState::Err(e) => return Err(anyhow!(e)),
            }
        }

        Ok(None)
    }

    pub fn key(&self) -> String {
        get_endpoint(
            self.local_ip.clone().unwrap().to_string(),
            self.mac_addr.clone().unwrap(),
        )
    }

    pub fn get_is_initialized(&self) -> bool {
        self.is_initialized.unwrap_or_default()
    }

    pub fn get_namespace(&self) -> String {
        self.namespace.clone().unwrap_or_default()
    }

    pub fn get_local_ip(&self) -> String {
        self.local_ip
            .clone()
            .map_or("".to_string(), |v| v.to_string())
    }

    pub async fn drop(&mut self) {
        if let Some(mut bridge) = self.bridge.clone() {
            info!("remove client {}", self.key());
            bridge.remove_client(self.key()).await;
        }
    }
}

impl WsClient<SplitSink<PWebSocketStream, PMessage>, SplitStream<PWebSocketStream>> {
    pub fn start_processing_to_client_msg(&mut self) {
        let mut receiver = self.receiver.take().unwrap();
        let mut ws_writer = self.ws_writer.take().unwrap();
        let msg_box = self.msg_box.clone();

        tokio::spawn(async move {
            let id_count = AtomicU64::new(1);
            while let Some(mut v) = receiver.recv().await {
                let buf = if let MsgKind::Response(_) = v.0.data {
                    Protocol::pack_response(v.0)
                } else {
                    v.0.id = id_count.fetch_add(1, Ordering::Relaxed);

                    if let Some(tx) = v.1 {
                        let tran = TransactionMsg::new(tx.clone(), v.0.id);
                        msg_box.insert(v.0.id, tran).await;
                    }
                    Protocol::pack_request(v.0)
                };

                ws_writer
                    .send(PMessage::Binary(buf))
                    .await
                    .expect("failed send message");
            }
        });
    }

    pub async fn auth(&mut self, namespace: String, secret: String) -> Result<AuthParams> {
        let mut ws_reader = self.ws_reader.take().unwrap();
        let mut ws_writer = self.ws_writer.take().unwrap();

        if let Some(msg) = ws_reader.next().await {
            let msg = match msg {
                Ok(v) => v,
                Err(e) => {
                    error!("failed read msg - {e}");
                    anyhow::bail!("failed read auth msg")
                }
            };
            if let PMessage::Binary(buf) = msg {
                let req = Protocol::unpack_request(buf)?;

                match req.data {
                    MsgKind::Request(MsgReqKind::Auth(v)) => {
                        if v.secret != secret {
                            anyhow::bail!("invalid secret");
                        }

                        self.namespace.replace(namespace);
                        self.local_ip.replace(v.agent_ip.parse().unwrap());
                        self.is_initialized.replace(v.is_initialized);
                        self.ws_reader.replace(ws_reader);

                        let _ = ws_writer
                            .send(PMessage::Binary(Protocol::pack_response(Msg {
                                id: 0,
                                data: MsgKind::Response(json!("ok")),
                            })))
                            .await?;

                        self.ws_writer.replace(ws_writer);

                        return Ok(v);
                    }
                    _ => anyhow::bail!("invalid auth msg(binary) type "),
                }
            }
        }
        anyhow::bail!("invalid auth msg(unknow) type")
    }

    pub async fn recv<T, F>(&mut self, handler: T)
    where
        T: FnOnce(MsgReqKind) -> F + Send + Sync + Clone + 'static,
        F: Future<Output = Value> + Send,
    {
        while let Some(msg) = self.ws_reader.as_mut().unwrap().next().await {
            let msg = match msg {
                std::result::Result::Ok(v) => v,
                Err(e) => {
                    error!("failed read msg - {e}");
                    return;
                }
            };

            let sender = self.sender.clone();
            if msg.is_binary() {
                let msg_box = self.msg_box.clone();
                let handler = handler.clone();
                tokio::spawn(async move {
                    if let PMessage::Binary(buf) = msg {
                        if Protocol::is_response(&buf) {
                            let resp = Protocol::unpack_response(buf)
                                .map_err(|e| error!("failed unpack_response - {e}"))
                                .unwrap();

                            if let Some(tx) = msg_box.get(&resp.id).await.map(|x| x.tx.clone()) {
                                if let MsgKind::Response(buf) = resp.data {
                                    let _ = tx
                                        .send(MsgState::Completed(buf))
                                        .await
                                        .map_err(|e| error!("failed send response - {e}"));
                                    return;
                                }
                                error!("invalid response format {:?}", resp);
                            }

                            return;
                        }

                        let resp = Protocol::unpack_request(buf)
                            .map(|msg| async move {
                                let id = msg.id;
                                if let MsgKind::Request(req) = msg.data {
                                    let resp = handler(req).await;
                                    Msg {
                                        id,
                                        data: MsgKind::Response(resp),
                                    }
                                } else {
                                    Msg {
                                        id,
                                        data: MsgKind::Response(json!("invalid data type")),
                                    }
                                }
                            })
                            .map_err(|e| error!("failed unpack_request -{e}"))
                            .unwrap()
                            .await;
                        let _ = sender
                            .send_timeout((resp, None), Duration::from_secs(1))
                            .await
                            .map_err(|e| error!("failed send message - {e}"));
                    }
                });
            }
        }
    }

    pub fn set_rw(
        &mut self,
        write: SplitSink<PWebSocketStream, PMessage>,
        read: SplitStream<PWebSocketStream>,
    ) -> &mut Self {
        self.ws_reader = Some(read);
        self.ws_writer = Some(write);
        self
    }
}

impl<W, R> Drop for WsClient<W, R> {
    fn drop(&mut self) {
        if let Some(_bridge) = self.bridge.clone() {

            // info!("remove client {}", self.key());
            // let fut = bridge.remove_client(self.key());
        }
    }
}

impl
    WsClient<
        SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    >
{
    pub async fn connect(&mut self, input: &str, secret: &str) -> Result<&mut Self> {
        let u = input.parse::<poem::http::Uri>()?;

        let mut req = if let Some(ref comet_secret) = self.comet_secret {
            ClientRequestBuilder::new(u)
                .with_header("Authorization", format!("Bearer {}", comet_secret))
        } else {
            ClientRequestBuilder::new(u)
        };
        if let Some(ref mac_addr) = self.mac_addr {
            req = req.with_header("X-Mac-Address", mac_addr)
        }

        if let Some(ref assign_user) = self.assign_user_option {
            req = req
                .with_header("X-Assign-Username", assign_user.username.clone())
                .with_header("X-Assign-Password", assign_user.password.clone());
        }

        if let Some(ref ssh_opt) = self.ssh_connection_option {
            req = req
                .with_header("X-Ssh-User", ssh_opt.user.clone())
                .with_header("X-Ssh-Password", ssh_opt.password.clone())
                .with_header("X-Ssh-Port", ssh_opt.port.to_string());
        }

        let (ws_stream, _b) = timeout(Duration::from_secs(5), connect_async(req))
            .await?
            .context("connect timeout")?;
        let (ws_writer, ws_reader) = ws_stream.split();
        self.ws_reader = Some(ws_reader);
        self.ws_writer = Some(ws_writer);

        let auth_resp = self
            .auth(self.is_initialized.unwrap_or_default(), secret.to_string())
            .await?;

        info!("success auth got response {auth_resp}");

        self.start_processing_to_server_msg();
        Ok(self)
    }

    fn start_processing_to_server_msg(&mut self) {
        let mut receiver = self.receiver.take().unwrap();
        let mut ws_writer = self.ws_writer.take().unwrap();
        let msg_box = self.msg_box.clone();

        tokio::spawn(async move {
            let id_count = AtomicU64::new(1);
            while let Some(mut v) = receiver.recv().await {
                let buf = if let MsgKind::Response(_) = v.0.data {
                    Protocol::pack_response(v.0)
                } else {
                    v.0.id = id_count.fetch_add(1, Ordering::Relaxed);
                    if let Some(tx) = v.1 {
                        let tran = TransactionMsg::new(tx.clone(), v.0.id);
                        msg_box.insert(v.0.id, tran).await;
                    }
                    Protocol::pack_request(v.0)
                };
                if let Err(e) = ws_writer.send(Message::Binary(buf)).await {
                    error!("failed send message - {e}");
                    return;
                }
            }
        });
    }

    pub async fn auth(&mut self, is_initialized: bool, secret: String) -> Result<Value> {
        let mut ws_writer = self.ws_writer.take().unwrap();
        let mut ws_reader = self.ws_reader.take().unwrap();

        let _ = timeout(
            Duration::from_secs(5),
            ws_writer.send(Message::Binary(Protocol::pack_request(Msg {
                id: 0,
                data: MsgKind::Request(MsgReqKind::Auth(AuthParams {
                    is_initialized,
                    agent_ip: self.local_ip.unwrap().to_string(),
                    secret,
                })),
            }))),
        )
        .await?
        .context("ws writer send timeout")?;
        self.ws_writer.replace(ws_writer);

        if let Some(msg) = timeout(Duration::from_secs(5), ws_reader.next())
            .await
            .context("wait auth responsee timeout")?
        {
            let msg = msg?;
            let msg = match msg {
                Message::Binary(v) => {
                    self.ws_reader.replace(ws_reader);
                    Protocol::unpack_response(v)
                }
                _ => anyhow::bail!("invalid ws msg type"),
            }?;

            match msg.data {
                MsgKind::Response(v) => return Ok(v),
                _ => anyhow::bail!("invalid auth response msg type"),
            };
        }
        anyhow::bail!("failed to auth connection")
    }

    pub async fn recv<T, F>(&mut self, handler: T)
    where
        T: FnOnce(MsgReqKind) -> F + Send + Sync + Clone + 'static,
        F: Future<Output = Value> + Send,
    {
        loop {
            let msg = match timeout(
                Duration::from_secs(90),
                self.ws_reader.as_mut().unwrap().next(),
            )
            .await
            {
                Ok(Some(Ok(v))) => v,
                Ok(Some(Err(e))) => {
                    error!("failed read msg - {e}");
                    return;
                }
                Err(e) => {
                    error!("read connection timeout {e}");
                    return;
                }
                _ => continue,
            };

            let sender = self.sender.clone();
            let handler = handler.clone();
            if msg.is_binary() {
                let msg_box = self.msg_box.clone();
                tokio::spawn(async move {
                    if let Message::Binary(buf) = msg {
                        if Protocol::is_response(&buf) {
                            let resp = Protocol::unpack_response(buf)
                                .map_err(|e| error!("failed unpack_response - {e}"))
                                .unwrap();

                            if let Some(tx) = msg_box.get(&resp.id).await.map(|x| x.tx.clone()) {
                                if let MsgKind::Response(buf) = resp.data {
                                    let _ = tx
                                        .send(MsgState::Completed(buf))
                                        .await
                                        .map_err(|e| error!("failed send response - {e}"));
                                    return;
                                }
                                error!("invalid response format {:?}", resp);
                            }

                            return;
                        }

                        let resp = Protocol::unpack_request(buf)
                            .map(|msg| async move {
                                let id = msg.id;
                                if let MsgKind::Request(req) = msg.data {
                                    let resp = handler(req).await;
                                    Msg {
                                        id,
                                        data: MsgKind::Response(resp),
                                    }
                                } else {
                                    Msg {
                                        id,
                                        data: MsgKind::Response(json!("invalid data type")),
                                    }
                                }
                            })
                            .map_err(|e| error!("failed unpack_request -{e}"))
                            .unwrap()
                            .await;
                        let _ = sender
                            .send_timeout((resp, None), Duration::from_secs(1))
                            .await
                            .map_err(|e| error!("failed send message - {e}"));
                    }
                });
            }
        }
    }
}

#[tokio::test]
async fn test_client() {
    use local_ip_address::local_ip;

    let mut client = WsClient::new(Some(Bridge::new()));
    client
        .set_namespace("default".to_string())
        .set_local_ip(local_ip().expect("failed get local ip"));

    client
        .connect("ws://127.0.0.1:3000/ws", "hello world")
        .await
        .expect("failed connect");

    for _v in 0..5 {
        client
            .send_msg(Msg {
                id: 2,
                data: MsgKind::Request(MsgReqKind::PullJobRequest(json!({"hello":"world"}))),
            })
            .await
            .expect("failed send message");
    }

    client.recv(|_x| async { todo!() }).await;
}
