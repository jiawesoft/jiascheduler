use anyhow::Result;
use clap::Parser;
use openapi::WebapiOptions;

/// A high-performance, scalable, dynamically configured job scheduler developed with rust
#[derive(Parser, Debug)]
#[command(author = "iwannay <772648576@qq.com>", about, version)]
struct WebapiArgs {
    /// if enable debug mode
    #[arg(short, long)]
    debug: bool,
    /// http server listen address, eg: "0.0.0.0:9090"
    #[arg(long)]
    bind_addr: Option<String>,

    /// where to read config file,
    /// you can temporarily overwrite the configuration file using command-line parameters
    #[arg(long, value_name = "FILE", default_value_t = String::from("~/.jiascheduler/console.toml"))]
    config: String,
    /// redis connect address, eg: "redis://:wang@127.0.0.1"
    #[arg(long)]
    redis_url: Option<String>,
    /// mysql connect address, eg: "mysql://root:root@localhost:3306/jiascheduler"
    #[arg(long)]
    database_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = WebapiArgs::parse();
    if args.debug {
        std::env::set_var("RUST_LOG", "debug");
    }
    tracing_subscriber::fmt::init();

    openapi::run(
        WebapiOptions {
            database_url: args.database_url,
            redis_url: args.redis_url,
            config_file: args.config,
            bind_addr: args.bind_addr,
        },
        None,
    )
    .await
}
