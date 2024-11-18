

use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
    },
};

use poem::{
    web::{
        websocket::{Message, WebSocket},
    }, IntoResponse,
};

use futures_util::{SinkExt, StreamExt};
use tracing::error;

pub struct WsServer {
    ws_channel: (Sender<String>, Option<Receiver<String>>),
    msg_channel: (Sender<String>, Option<Receiver<String>>),
}

impl Default for WsServer {
    fn default() -> Self {
        Self::new()
    }
}

impl WsServer {
    pub fn new() -> Self {
        let (ws_sender, ws_receiver) = mpsc::channel::<String>(10);
        let (msg_sender, msg_receiver) = mpsc::channel::<String>(10);
        Self {
            ws_channel: (ws_sender, Some(ws_receiver)),
            msg_channel: (msg_sender, Some(msg_receiver)),
        }
    }

    fn msg_sender(&self) -> Sender<String> {
        self.msg_channel.0.clone()
    }

    pub async fn recv(&mut self) -> Receiver<String> {
        self.msg_channel.1.take().expect("invalid msg receiver")
    }

    async fn poll<F>(&mut self, mut handler: F)
    where
        F: FnMut(String) + Send + Sync + 'static,
    {
        let mut msg_receiver = self.msg_channel.1.take().expect("invalid msg receiver");
        match tokio::spawn(async move {
            while let Some(v) = msg_receiver.recv().await {
                handler(v);
            }
        })
        .await
        {
            Ok(_) => todo!(),
            Err(e) => error!("{e}"),
        }
    }

    pub async fn serve_ws(&mut self, ws: WebSocket) -> impl IntoResponse {
        let msg_sender = self.msg_channel.0.clone();

        let mut ws_receiver = self.ws_channel.1.take().expect("invalid receiver");

        ws.on_upgrade(move |socket| async move {
            let (mut sink, mut stream) = socket.split();

            tokio::spawn(async move {
                while let Some(Ok(msg)) = stream.next().await {
                    if let Message::Text(text) = msg {
                        match msg_sender.send(format!("read: {text}")).await {
                            Err(e) => {
                                error!("{e}");
                                break;
                            }
                            Ok(_) => todo!(),
                        }
                    }
                }
            });

            tokio::spawn(async move {
                while let Some(msg) = ws_receiver.recv().await {
                    if sink.send(Message::Text(msg)).await.is_err() {
                        error!("err!");
                        break;
                    }
                }
            });
        })
    }
}
