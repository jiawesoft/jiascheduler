use std::{future::Future, pin::Pin, str::FromStr, sync::Arc};

use anyhow::{Result, anyhow};
use chrono::{Local, Utc};
use croner::{Cron, parser::CronParser};
use tokio::sync::RwLock;
pub mod macros;

pub async fn async_batch_do<I, T, F>(data: Vec<I>, handler: F) -> Vec<Result<T>>
where
    F: 'static + Send + Sync + Clone + Fn(I) -> Pin<Box<dyn Future<Output = Result<T>> + Send>>,
    I: Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let data_len = data.len();
    let locked_data = Arc::new(RwLock::new(data));
    let locked_outputs = Arc::new(RwLock::new(Vec::with_capacity(data_len)));
    let queue_len = if data_len > 500 { 500 } else { data_len };
    let mut tasks = Vec::with_capacity(queue_len);

    for _ in 0..queue_len {
        let locked_data = locked_data.clone();
        let locked_outputs = locked_outputs.clone();
        let handler = handler.clone();
        tasks.push(tokio::spawn(async move {
            loop {
                let mut queue = locked_data.write().await;
                if let Some(val) = queue.pop() {
                    drop(queue);
                    let ret = handler(val).await;
                    let mut outputs = locked_outputs.write().await;
                    outputs.push(ret);
                } else {
                    return;
                }
            }
        }));
    }

    for task in tasks {
        let _ = task.await;
    }

    let outputs = locked_outputs.read().await;

    let mut ret = Vec::new();

    outputs.iter().for_each(|v| {
        ret.push(match v {
            Ok(v) => Ok(v.to_owned()),
            Err(e) => Err(anyhow!("{e}")),
        })
    });

    ret
}

#[tokio::test]
async fn test_async_queue_do() {
    use std::time::Duration;
    use tokio::time::sleep;
    unsafe {
        std::env::set_var("RUST_LOG", "debug");
    }

    tracing_subscriber::fmt::init();
    let data = 1..100;

    #[derive(Debug, Clone)]
    pub struct QueueResult {
        _val: i32,
    }

    let ret = async_batch_do(data.clone().collect(), |v| {
        Box::pin(async move {
            sleep(Duration::from_secs(1)).await;
            Ok(QueueResult { _val: v })
        })
    })
    .await;

    println!("result:{:?}, len: {}", ret, ret.len(),)
}

pub fn check_timer_expr(timezone: &str, expr: &str) -> Result<Vec<String>> {
    let parsed_expr = match CronParser::builder()
        .seconds(croner::parser::Seconds::Required)
        .dom_and_dow(true)
        .build()
        .parse(&expr)
    {
        Ok(_) => expr.to_string(),
        Err(e1) => match english_to_cron::str_cron_syntax(expr) {
            Ok(english_to_cron) => {
                if english_to_cron != expr {
                    if english_to_cron == "0 * * * * ? *" {
                        anyhow::bail!("failed parse {} to cron expr, {}", expr, e1.to_string())
                    } else {
                        // english-to-cron adds the year field which we can't put off (currently)
                        let cron = english_to_cron
                            .split(' ')
                            .take(6)
                            .collect::<Vec<_>>()
                            .join(" ");
                        cron
                    }
                } else {
                    expr.to_string()
                }
            }
            Err(e2) => {
                anyhow::bail!(
                    "failed parse cron expr, 1.{}, 2.{}",
                    e1.to_string(),
                    e2.to_string()
                )
            }
        },
    };

    let parsed_cron = match Cron::from_str(&parsed_expr) {
        Err(e) => anyhow::bail!("failed build cron, {}", e.to_string()),
        Ok(v) => v,
    };

    let mut now = Local::now();
    let mut next_exec_times: Vec<String> = vec![];

    for _ in 0..10 {
        let next_time = match parsed_cron.find_next_occurrence(&now, false) {
            Err(e) => anyhow::bail!("failed find next execution time, {}", e.to_string()),
            Ok(v) => {
                now = v.clone();
                match timezone {
                    "local" => v
                        .with_timezone(&Local)
                        .format("%Y/%m/%d %H:%M:%S")
                        .to_string(),
                    "utc" | _ => v
                        .with_timezone(&Utc)
                        .format("%Y/%m/%d %H:%M:%S")
                        .to_string(),
                }
            }
        };
        next_exec_times.push(next_time);
    }

    Ok(next_exec_times)
}
