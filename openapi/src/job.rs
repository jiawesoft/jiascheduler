use std::time::Duration;

use anyhow::{Context, Result};
use automate::{
    bridge::msg::{AgentOfflineParams, AgentOnlineParams, HeartbeatParams},
    bus::{Bus, Msg},
};

use tokio::time::sleep;
use tracing::{error, info};

use crate::AppState;

async fn heartbeat(mut state: AppState, msg: HeartbeatParams) -> Result<()> {
    state
        .service()
        .instance
        .update_instance(msg.mac_addr, msg.source_ip)
        .await?;

    if state.can_execute().await {
        let svc = state.service();
        let _ = svc
            .instance
            .offline_inactive_instance(600)
            .await
            .context("failed offline inactive instance")
            .map_err(|e| error!("{e:?}"));
    }
    Ok(())
}

async fn agent_online(state: AppState, msg: AgentOnlineParams) -> Result<()> {
    info!("{}:{}:{} online", msg.agent_ip, msg.namespace, msg.mac_addr);
    let mut svc = state.service();

    svc.instance
        .update_status(
            Some(msg.namespace.clone()),
            msg.agent_ip.clone(),
            msg.mac_addr.clone(),
            1,
            msg.secret_header.assign_user,
            msg.secret_header.ssh_connection_params,
        )
        .await?;

    if !msg.is_initialized {
        info!(
            "start initialize runnable job on {}:{}",
            msg.agent_ip, msg.namespace,
        );
        if let Err(e) = svc
            .job
            .dispatch_runnable_job_to_endpoint(
                msg.namespace.clone(),
                msg.agent_ip.clone(),
                msg.mac_addr.clone(),
            )
            .await
        {
            error!(
                "failed dispatch_runnable_job_to_endpoint, {}",
                e.to_string()
            );
        }
    }

    Ok(())
}

async fn agent_offline(state: AppState, msg: AgentOfflineParams) -> Result<()> {
    info!("{}:{} offline", msg.agent_ip, msg.mac_addr,);

    Ok(state
        .service()
        .instance
        .update_status(None, msg.agent_ip, msg.mac_addr, 0, None, None)
        .await?)
}

pub async fn start(state: AppState) -> Result<()> {
    let bus = Bus::new(state.redis().clone());

    tokio::spawn(async move {
        loop {
            let ret = bus
                .recv(|_key, msg| {
                    let state = state.clone();
                    Box::pin(async move {
                        match msg {
                            Msg::UpdateJob(v) => {
                                let _ = state.service().job.update_job_status(v).await?;
                            }
                            Msg::Heartbeat(v) => {
                                let _ = heartbeat(state.clone(), v).await?;
                            }
                            Msg::AgentOnline(msg) => agent_online(state.clone(), msg).await?,
                            Msg::AgentOffline(msg) => agent_offline(state.clone(), msg).await?,
                        };
                        Ok(())
                    })
                })
                .await;
            if let Err(e) = ret {
                error!("failed to recv bus msg - {e}");
                sleep(Duration::from_millis(500)).await;
            }
            info!("restart recv bus msg");
        }
    });
    Ok(())
}
