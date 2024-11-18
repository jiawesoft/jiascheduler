use anyhow::Result;
use automate::{
    bridge::msg::{AgentOfflineParams, AgentOnlineParams, HeartbeatParams},
    bus::{Bus, Msg},
};
use tracing::{error, info};

use crate::{entity::instance, AppState};

async fn heartbeat(state: AppState, msg: HeartbeatParams) -> Result<()> {
    state
        .service()
        .instance
        .update_instance(instance::Model {
            ip: msg.source_ip,
            namespace: msg.namespace,
            status: 1,
            ..Default::default()
        })
        .await?;
    Ok(())
}

async fn agent_online(state: AppState, msg: AgentOnlineParams) -> Result<()> {
    info!(
        "namespace: {}, agent_ip: {} online",
        msg.namespace, msg.agent_ip
    );
    let mut svc = state.service();
    if !msg.is_initialized {
        info!(
            "start initialize runnable job on {}:{}",
            msg.agent_ip, msg.namespace,
        );
        if let Err(e) = svc
            .job
            .dispatch_runnable_job_to_endpoint(msg.namespace.clone(), msg.agent_ip.clone())
            .await
        {
            error!(
                "failed dispatch_runnable_job_to_endpoint, {}",
                e.to_string()
            );
        }
    }

    Ok(svc
        .instance
        .update_status(
            msg.namespace,
            msg.agent_ip,
            1,
            msg.secret_header.assign_user,
            msg.secret_header.ssh_connection_params,
        )
        .await?)
}

async fn agent_offline(state: AppState, msg: AgentOfflineParams) -> Result<()> {
    info!(
        "namespace: {}, agent_ip: {} offline",
        msg.namespace, msg.agent_ip
    );

    Ok(state
        .service()
        .instance
        .update_status(msg.namespace, msg.agent_ip, 0, None, None)
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
            }
            info!("restart recv bus msg");
        }
    });
    Ok(())
}
