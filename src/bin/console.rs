use anyhow::Result;
use clap::Parser;
use openapi::WebapiOptions;

/// A high-performance, scalable, dynamically configured job scheduler developed with rust
#[derive(Parser, Debug)]
#[command(
    author = "iwannay <772648576@qq.com>",
    about = "A high-performance, scalable, dynamically configured job scheduler developed with rust",
    version
)]
struct WebapiArgs {
    /// if enable debug mode
    #[arg(short, long)]
    debug: bool,
    /// http server listen address, eg: "0.0.0.0:9090"
    #[arg(long)]
    bind_addr: Option<String>,

    /// Set log level, eg: "trace", "debug", "info", "warn", "error" etc.
    #[arg(long, default_value_t = String::from("error"))]
    log_level: String,

    /// where to read config file,
    /// you can temporarily overwrite the configuration file using command-line parameters
    #[arg(long, value_name = "FILE", default_value_t = String::from("~/.jiascheduler/console.toml"))]
    config: String,
    /// redis connect address, eg: "redis://:wang@127.0.0.1"
    /// can be used to override configuration items in the configuration file
    #[arg(long)]
    redis_url: Option<String>,
    /// mysql connect address, eg: "mysql://root:root@localhost:3306/jiascheduler"
    /// can be used to override configuration items in the configuration file
    #[arg(long)]
    database_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = WebapiArgs::parse();
    unsafe {
        std::env::set_var("RUST_LOG", args.log_level);
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
