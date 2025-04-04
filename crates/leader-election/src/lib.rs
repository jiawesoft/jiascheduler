use nanoid::nanoid;
use redis::{AsyncCommands, Client, RedisResult};
use tokio::time::sleep;

use std::{pin::Pin, time::Duration};

pub struct LeaderElection {
    redis_client: Client,
    key: String,
    id: String,
    ttl: i64,
    check_interval: Duration,
}

impl LeaderElection {
    pub fn new(client: Client, key: &str, ttl: i64) -> RedisResult<Self> {
        Ok(Self {
            redis_client: client,
            key: key.to_string(),
            id: format!("{}", nanoid!()),
            ttl,
            check_interval: Duration::from_secs((ttl / 2) as u64),
        })
    }

    async fn acquire_leadership(&mut self) -> RedisResult<bool> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let acquired: bool = conn.set_nx(&self.key, &self.id).await?;
        if acquired {
            conn.expire::<_, ()>(&self.key, self.ttl).await?;
            return Ok(true);
        }

        let current_id: Option<String> = conn.get(&self.key).await?;
        if current_id.as_ref() == Some(&self.id) {
            conn.expire::<_, ()>(&self.key, self.ttl).await?;
            return Ok(true);
        }

        Ok(false)
    }

    pub async fn run_election<F>(&mut self, mut leader_callback: F) -> RedisResult<()>
    where
        F: Sync + Send + FnMut(bool) -> Pin<Box<dyn Future<Output = ()> + Send>>,
    {
        let mut is_leader = false;

        loop {
            match self.acquire_leadership().await {
                Ok(acquired) => {
                    if acquired != is_leader {
                        is_leader = acquired;
                        leader_callback(is_leader).await;
                    }
                    if is_leader {
                        sleep(self.check_interval).await;
                    } else {
                        sleep(Duration::from_secs(1)).await;
                    }
                }
                Err(e) => {
                    eprintln!("Leader election error: {:?}", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}
