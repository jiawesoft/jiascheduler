use std::net::SocketAddr;

use anyhow::Result;
use automate::comet::{
    handler::{self, middleware::bearer_auth},
    Comet,
};
use clap::Parser;
use poem::{get, listener::TcpListener, post, EndpointExt, Route, Server};

#[derive(Parser, Debug)]
#[command(
    author = "iwannay <772648576@qq.com>",
    about = "A high-performance, scalable, dynamically configured job scheduler developed with rust",
    version = "0.0.1",
    long_about = None
)]
struct CometArgs {
    /// if enable debug mode
    #[arg(short, long)]
    debug: bool,
    #[arg(short, long, default_value_t = String::from("0.0.0.0:3000"))]
    bind: String,
    #[arg(short,default_value_t = String::from("redis://:wang@127.0.0.1"))]
    redis_url: String,
    #[arg(long, default_value_t = String::from("rYzBYE+cXbtdMg=="))]
    secret: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CometArgs::parse();
    if args.debug {
        std::env::set_var("RUST_LOG", "debug");
    }
    tracing_subscriber::fmt::init();

    let redis_client = redis::Client::open(args.redis_url).expect("failed connect to redis");

    let port = args.bind.parse::<SocketAddr>()?.port();
    let comet = Comet::new(redis_client, port, args.secret.clone());

    let app = Route::new()
        .at(
            "/dispatch",
            post(
                handler::dispatch
                    .with(bearer_auth(&args.secret))
                    .data(comet.clone()),
            ),
        )
        .at(
            "runtime/action",
            post(
                handler::runtime_action
                    .with(bearer_auth(&args.secret))
                    .data(comet.clone()),
            ),
        )
        .at(
            "/file/get/:filename",
            get(handler::get_file
                .with(bearer_auth(&args.secret))
                .data(comet.clone())),
        )
        .at(
            "/evt/:namespace",
            get(handler::ws
                .with(bearer_auth(&args.secret))
                .data(comet.clone())),
        )
        .at(
            "/ssh/register/:ip",
            handler::ssh_register
                .with(bearer_auth(&args.secret))
                .data(comet.clone()),
        )
        .at(
            "/ssh/tunnel/:ip",
            handler::proxy_ssh
                .with(bearer_auth(&args.secret))
                .data(comet.clone()),
        )
        .at(
            "/sftp/tunnel/read-dir",
            handler::sftp_read_dir
                .with(bearer_auth(&args.secret))
                .data(comet.clone()),
        )
        .at(
            "/sftp/tunnel/upload",
            handler::sftp_upload
                .with(bearer_auth(&args.secret))
                .data(comet.clone()),
        )
        .at(
            "/sftp/tunnel/remove",
            handler::sftp_remove
                .with(bearer_auth(&args.secret))
                .data(comet.clone()),
        )
        .at(
            "/sftp/tunnel/download",
            handler::sftp_download
                .with(bearer_auth(&args.secret))
                .data(comet.clone()),
        );

    Ok(Server::new(TcpListener::bind(args.bind)).run(app).await?)
}
