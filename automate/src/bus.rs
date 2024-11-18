use std::pin::Pin;

use anyhow::Result;
use futures::Future;
use local_ip_address::local_ip;
use redis::{
    from_redis_value,
    streams::{StreamReadOptions, StreamReadReply},
    AsyncCommands, Client,
};
use redis_macros::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};

use tracing::{error, info, warn};

use crate::bridge::msg::{AgentOfflineParams, AgentOnlineParams, HeartbeatParams, UpdateJobParams};

#[derive(Debug, Serialize, Deserialize, FromRedisValue, ToRedisArgs)]
pub enum Msg {
    UpdateJob(UpdateJobParams),
    Heartbeat(HeartbeatParams),
    AgentOnline(AgentOnlineParams),
    AgentOffline(AgentOfflineParams),
}

#[derive(Clone)]
pub struct Bus {
    pub redis_client: Client,
}

impl Bus {
    pub const JOB_TOPIC: &'static str = "jiascheduler:job:event";
    pub const CONSUMER_GROUP: &'static str = "jiascheduler-group";

    pub fn new(redis_client: Client) -> Self {
        Self { redis_client }
    }

    pub async fn update_job(&self, msg: UpdateJobParams) -> Result<String> {
        self.send_msg(&[("event", Msg::UpdateJob(msg))]).await
    }

    pub async fn heartbeat(&self, msg: HeartbeatParams) -> Result<String> {
        self.send_msg(&[("event", Msg::Heartbeat(msg))]).await
    }

    pub async fn agent_online(&self, msg: AgentOnlineParams) -> Result<String> {
        self.send_msg(&[("event", Msg::AgentOnline(msg))]).await
    }

    pub async fn agent_offline(&self, msg: AgentOfflineParams) -> Result<String> {
        self.send_msg(&[("event", Msg::AgentOffline(msg))]).await
    }

    pub async fn send_msg<'a>(&self, items: &'a [(&'a str, Msg)]) -> Result<String> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let v: String = conn.xadd(Self::JOB_TOPIC, "*", items).await?;
        Ok(v)
    }

    pub async fn recv(
        &self,
        mut cb: impl Sync
            + Send
            + FnMut(String, Msg) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>,
    ) -> Result<String> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let ret: String = conn
            .xgroup_create_mkstream(Self::JOB_TOPIC, Self::CONSUMER_GROUP, "$")
            .await
            .map_or_else(
                |e| {
                    warn!("failed create stream group - {}", e);
                    "".to_string()
                },
                |v| v,
            );

        info!("create stream group {}", ret);

        let opts = StreamReadOptions::default()
            .group(Self::CONSUMER_GROUP, local_ip()?.to_string())
            .block(50)
            .count(100);

        loop {
            let ret: StreamReadReply = conn
                .xread_options(&[Self::JOB_TOPIC], &[">"], &opts)
                .await?;

            for stream_key in ret.keys {
                let msg_key = stream_key.key;

                for stream_id in stream_key.ids {
                    for (k, v) in stream_id.map {
                        let ret = match from_redis_value::<Msg>(&v) {
                            Ok(msg) => cb(k, msg).await,
                            Err(e) => {
                                error!("failed to parse redis val - {e}");
                                Ok(())
                            }
                        };

                        if let Err(e) = ret {
                            error!("failed to handle msg - {e}");
                        }

                        let _: i32 = conn
                            .xack(
                                msg_key.clone(),
                                Self::CONSUMER_GROUP,
                                &[stream_id.id.clone()],
                            )
                            .await
                            .map_or_else(
                                |v| {
                                    error!("faile to exec xack - {}", v);
                                    0
                                },
                                |v| v,
                            );
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn test_bus() {
    let redis_client =
        redis::Client::open("redis://:wang@127.0.0.1").expect("failed connect to redis");
    let bus = Bus::new(redis_client);
    bus.send_msg(&[(
        "event",
        Msg::UpdateJob(UpdateJobParams {
            exit_code: Some(1),
            ..Default::default()
        }),
    )])
    .await
    .unwrap();

    bus.send_msg(&[(
        "event",
        Msg::UpdateJob(UpdateJobParams {
            exit_code: Some(2),
            ..Default::default()
        }),
    )])
    .await
    .unwrap();

    bus.recv(|key, val| {
        Box::pin(async move {
            println!("key:{key} val:{}", serde_json::to_string(&val).unwrap());
            Ok(())
        })
    })
    .await
    .unwrap();
}
