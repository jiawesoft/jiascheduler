use automate::bridge::{Bridge, client::WsClient};
use futures_util::{
    StreamExt,
    stream::{SplitSink, SplitStream},
};
use local_ip_address::local_ip;
use poem::{
    EndpointExt, IntoResponse, Route, Server, get, handler,
    listener::TcpListener,
    web::{
        Data, Html,
        websocket::{Message, WebSocket, WebSocketStream},
    },
};
use tracing::info;

#[handler]
fn index() -> Html<&'static str> {
    Html(
        r###"
    <body>
        <form id="loginForm">
            Name: <input id="nameInput" type="text" />
            <button type="submit">Login</button>
        </form>
        
        <form id="sendForm" hidden>
            Text: <input id="msgInput" type="text" />
            <button type="submit">Send</button>
        </form>
        
        <textarea id="msgsArea" cols="50" rows="30" hidden></textarea>
    </body>
    <script>
        let ws;
        const loginForm = document.querySelector("#loginForm");
        const sendForm = document.querySelector("#sendForm");
        const nameInput = document.querySelector("#nameInput");
        const msgInput = document.querySelector("#msgInput");
        const msgsArea = document.querySelector("#msgsArea");
        
        nameInput.focus();

        loginForm.addEventListener("submit", function(event) {
            event.preventDefault();
            loginForm.hidden = true;
            sendForm.hidden = false;
            msgsArea.hidden = false;
            msgInput.focus();
            ws = new WebSocket("ws://127.0.0.1:3000/ws/" + nameInput.value);
            ws.onmessage = function(event) {
                msgsArea.value += event.data + "\r\n";
            }
        });
        
        sendForm.addEventListener("submit", function(event) {
            event.preventDefault();
            ws.send(msgInput.value);
            msgInput.value = "";
        });

    </script>
    "###,
    )
}

#[handler]
fn ws(ws: WebSocket, mut bridge: Data<&Bridge>) -> impl IntoResponse {
    let mut bridge = bridge.clone();
    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();
        let mut client: WsClient<
            SplitSink<WebSocketStream, Message>,
            SplitStream<WebSocketStream>,
        > = WsClient::new(Some(bridge.clone()));
        client
            .set_namespace(String::from("default"))
            .set_local_ip(local_ip().expect("failed get local ip"));
        bridge
            .append_client("hello".to_string(), client.sender())
            .await;
        client.set_rw(sink, stream);
        client.start_processing_to_client_msg();
        client.recv(|_x| async { todo!() }).await;
    })
}

#[tokio::main]
async fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        unsafe {
            std::env::set_var("RUST_LOG", "comet=debug");
        }
    }
    tracing_subscriber::fmt::init();
    info!("start es example");
    let bridge = Bridge::new();
    let app = Route::new().at("/", get(index)).at("/ws", ws.data(bridge));

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
        .expect("failed start server");

    println!("hello world");
}
