use std::{future::Future, pin::Pin, sync::Arc};

use anyhow::{anyhow, Result};
use tokio::sync::RwLock;

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

    std::env::set_var("RUST_LOG", "debug");
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
