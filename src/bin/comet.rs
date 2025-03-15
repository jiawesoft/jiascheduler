use std::net::SocketAddr;

use anyhow::Result;
use automate::comet::{
    self,
    handler::{self, middleware::bearer_auth},
    Comet, CometOptions,
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

    comet::run(
        CometOptions {
            redis_url: args.redis_url,
            bind_addr: args.bind,
            secret: args.secret,
        },
        None,
    )
    .await
}
