use std::{sync::Arc, time::Duration};

use tokio::{select, time::sleep};
use watchexec_supervisor::{
    command::{Command, Program, SpawnOptions},
    job::start_job,
};

async fn supervisor_test() {
    let code = r#"date;
    echo hello world
    sleep 5 &
    echo end"#;

    let (job, task) = start_job(Arc::new(Command {
        program: Program::Exec {
            prog: "/usr/bin/bash".into(),
            args: vec!["-c".into(), code.into()],
        }
        .into(),
        options: SpawnOptions {
            grouped: true,
            reset_sigmask: true,
        },
    }));

    job.start().await;
    job.set_error_handler(|v| {
        let e = v.get().unwrap();
        println!("error: {e}");
    });

    let clone_job = job.clone();

    tokio::spawn(async move {
        loop {
            select! {
                _v = clone_job.to_wait() => {
                    if clone_job.is_dead() {
                        return;
                    }
                    sleep(Duration::from_secs(1)).await;
                    clone_job.start().await;
                }
            }
        }
    });

    sleep(Duration::from_secs(10)).await;
    job.stop();
    job.delete_now();

    let _ = task.await.expect("failed to wait join finished");
    sleep(Duration::from_secs(5)).await;
}

#[tokio::main]
async fn main() {
    supervisor_test().await;
}
