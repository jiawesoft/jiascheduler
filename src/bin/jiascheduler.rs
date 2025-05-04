use std::sync::Arc;

use anyhow::Result;
use automate::{
    comet::{self, CometOptions},
    scheduler::{
        Scheduler,
        types::{AssignUserOption, SshConnectionOption},
    },
};
use clap::Parser;
use openapi::WebapiOptions;
use service::config::Conf;
use tokio::sync::{Mutex, oneshot::channel};
use tracing::{error, info};

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
    console_bind_addr: Option<String>,

    /// Set log level, eg: "info", "debug", "warn", "error" etc.
    #[arg(long, default_value_t = String::from("error"))]
    log_level: String,

    /// Comet server listen address, eg: "0.0.0.0:3000"
    #[arg(short, long, default_value_t = String::from("0.0.0.0:3000"))]
    comet_bind_addr: String,

    #[arg(short, long, default_value_t = String::from("default"))]
    namespace: String,
    /// Directory for saving job execution logs
    #[arg(long, default_value_t = String::from("./log"))]
    output_dir: String,
    /// Set the login user of the instance for SSH remote connection
    #[arg(long)]
    ssh_user: Option<String>,
    /// Set the login user's password of the instance for SSH remote connection
    #[arg(long)]
    ssh_password: Option<String>,
    /// Set the port of this instance for SSH remote connection
    #[arg(long)]
    ssh_port: Option<u16>,

    /// Assign this instance to a user and specify their username
    #[arg(long)]
    assign_username: Option<String>,
    /// Assign this instance to a user and specify their password
    #[arg(long)]
    assign_password: Option<String>,

    /// where to read config file,
    /// you can temporarily overwrite the configuration file using command-line parameters
    #[arg(long, value_name = "FILE", default_value_t = String::from("~/.jiascheduler/console.toml"))]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = WebapiArgs::parse();
    unsafe {
        std::env::set_var("RUST_LOG", args.log_level);
        if args.debug {
            std::env::set_var("RUST_LOG", "debug");
        }
    }

    tracing_subscriber::fmt::init();

    let (console_tx, console_rx) = channel::<Conf>();
    let (comet_tx, comet_rx) = channel::<()>();

    let console_conf: Arc<Mutex<Option<Conf>>> = Arc::new(Mutex::new(None));
    let console_conf_clone = console_conf.clone();
    let comet_bind_addr = args.comet_bind_addr.clone();

    tokio::spawn(async move {
        let conf = console_rx.await.unwrap();
        console_conf_clone.lock().await.replace(conf.clone());
        info!("starting comet");
        comet::run(
            CometOptions {
                redis_url: conf.redis_url,
                bind_addr: comet_bind_addr.clone(),
                secret: conf.comet_secret,
            },
            Some(comet_tx),
        )
        .await
        .expect("failed to start comet server");
    });

    tokio::spawn(async move {
        comet_rx
            .await
            .expect("failed to receive comet server signal");
        let binding = console_conf.lock().await;
        let conf = binding.as_ref().unwrap();
        let mut scheduler = Scheduler::new(
            args.namespace,
            vec![format!("ws://{}", args.comet_bind_addr)],
            conf.comet_secret.to_string(),
            args.output_dir,
            SshConnectionOption::build(args.ssh_user, args.ssh_password, args.ssh_port),
            AssignUserOption::build(args.assign_username, args.assign_password),
        );
        info!("starting agent");
        if let Err(e) = scheduler.connect_comet().await {
            error!("failed connect to comet - {e}");
        }

        scheduler.run().await.expect("failed to start scheduler");
    });

    openapi::run(
        WebapiOptions {
            database_url: None,
            redis_url: None,
            config_file: args.config,
            bind_addr: args.console_bind_addr,
        },
        Some(console_tx),
    )
    .await
}
