// mod bridge;
pub mod client;
pub mod msg;
pub mod protocol;
// pub mod server;

use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Ok, Result};
use serde_json::Value;
use tokio::{
    sync::{
        mpsc::{self, Sender},
        Mutex,
    },
    time::timeout,
};
use tracing::info;

use crate::bridge::msg::Msg;

use self::msg::{MsgKind, MsgReqKind, MsgState};

#[derive(Clone)]
pub struct Bridge {
    // server: WsServer,
    server_clients: Arc<Mutex<HashMap<String, Sender<(Msg, Option<Sender<MsgState>>)>>>>,
}

impl Default for Bridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Bridge {
    pub fn new() -> Self {
        Self {
            // server: WsServer::new(),
            server_clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn append_client(
        &mut self,
        key: impl Into<String>,
        client: Sender<(Msg, Option<Sender<MsgState>>)>,
    ) {
        self.server_clients.lock().await.insert(key.into(), client);
    }

    pub async fn remove_client(&mut self, key: String) {
        self.server_clients.lock().await.remove(&key);
    }

    pub async fn send_msg(&self, key: &str, data: MsgReqKind) -> Result<Value> {
        let msg = Msg {
            id: 0,
            data: MsgKind::Request(data),
        };
        let (tx, mut rx) = mpsc::channel::<MsgState>(1);

        match self.server_clients.lock().await.get(key) {
            Some(sender) => sender.send((msg, Some(tx.clone()))).await?,
            None => return Err(anyhow::anyhow!("not found client {}", key)),
        }

        let resp = timeout(Duration::from_secs(90), rx.recv())
            .await?
            .context("receive message timeout")?;

        return match resp {
            MsgState::Completed(v) => Ok(v),
            MsgState::Err(e) => Err(anyhow!(e)),
        };
    }

    pub fn handle_msg(&mut self, msg: String) -> String {
        info!("handle msg {msg}");

        format!("pong {msg}")
    }

    pub fn handle_msg2(&mut self, msg: Value) -> Result<String> {
        info!("handle msg {msg}");

        // match serde_json::from_value::<Msg>(msg)? {
        //     Msg::DispathJobMsg(msg) => todo!(),
        // }

        // format!("pong {msg}");
        todo!()
    }

    // pub async fn poll<F>(&mut self, mut handler: F)
    // where
    //     F: FnMut(String) + Send + Sync + 'static,
    // {
    //     let mut msg_receiver = self.server.recv().await;
    //     match tokio::spawn(async move {
    //         while let Some(v) = msg_receiver.recv().await {
    //             handler(v);
    //         }
    //     })
    //     .await
    //     {
    //         Ok(_) => todo!(),
    //         Err(e) => error!("{e}"),
    //     }
    // }
}
