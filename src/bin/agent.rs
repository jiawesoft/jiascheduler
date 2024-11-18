use anyhow::Result;
use clap::Parser;

use tracing::error;

use automate::scheduler::{
    types::{AssignUserOption, SshConnectionOption},
    Scheduler,
};

#[derive(Parser, Debug)]
#[command(
    author = "iwannay <772648576@qq.com>",
    about = "A high-performance, scalable, dynamically configured job scheduler developed with rust",
    version = "0.0.1",
    long_about = None
)]
struct AgentArgs {
    /// If enable debug mode
    #[arg(short, long)]
    debug: bool,
    #[arg(short, long, default_value_t = String::from("0.0.0.0:3001"))]
    bind: String,
    #[arg(long, default_values_t = vec![String::from("ws://127.0.0.1:3000")])]
    comet_addr: Vec<String>,
    /// Directory for saving job execution logs
    #[arg(long, default_value_t = String::from("./log"))]
    output_dir: String,
    #[arg(long, default_value_t = String::from("rYzBYE+cXbtdMg=="))]
    comet_secret: String,
    #[arg(short, long, default_value_t = String::from("default"))]
    namespace: String,
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = AgentArgs::parse();
    if args.debug {
        std::env::set_var("RUST_LOG", "debug");
    }
    tracing_subscriber::fmt::init();

    let mut scheduler = Scheduler::new(
        args.namespace,
        args.comet_addr,
        args.comet_secret,
        args.output_dir,
        SshConnectionOption::build(args.ssh_user, args.ssh_password, args.ssh_port),
        AssignUserOption::build(args.assign_username, args.assign_password),
    );

    if let Err(e) = scheduler.connect_comet().await {
        error!("failed connect to comet - {e}");
    }

    scheduler.run().await
}
