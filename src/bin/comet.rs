use anyhow::Result;
use automate::comet::{self, CometOptions};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    author = "iwannay <772648576@qq.com>",
    about = "A high-performance, scalable, dynamically configured job scheduler developed with rust",
    version
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

    /// Set log level, eg: "trace", "debug", "info", "warn", "error" etc.
    #[arg(long, default_value_t = String::from("error"))]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CometArgs::parse();
    unsafe {
        std::env::set_var("RUST_LOG", args.log_level);
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
