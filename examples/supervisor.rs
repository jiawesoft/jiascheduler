use std::{sync::Arc, time::Duration};

use tokio::{select, time::sleep};
use watchexec_supervisor::{
    command::{Command, Program, SpawnOptions},
    job::start_job,
};

async fn supervisor_test() {
    let (job, task) = start_job(Arc::new(Command {
        program: Program::Exec {
            prog: "/usr/bin/bash".into(),
            args: vec!["-c".into(), r#"date;echo hello;sleep 10 &"#.into()],
        }
        .into(),
        options: SpawnOptions {
            grouped: false,
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
                    sleep(Duration::from_secs(1)).await;
                    clone_job.start().await;
                }
            }
        }
    });

    job.to_wait().await;

    let _ = task.await.expect("failed to wait join finished");
}

#[tokio::main]
async fn main() {
    supervisor_test().await;
}
