use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use automate::{
    bridge::msg::{AgentOfflineParams, AgentOnlineParams, HeartbeatParams},
    bus::{Bus, Msg},
};

use leader_election::LeaderElection;
use tokio::{sync::RwLock, time::sleep};
use tracing::{error, info};

use crate::AppState;

async fn heartbeat(state: AppState, msg: HeartbeatParams) -> Result<()> {
    state
        .service()
        .instance
        .set_instance_online(msg.mac_addr, msg.source_ip)
        .await?;

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
        svc.job
            .fix_running_status(&msg.agent_ip, &msg.mac_addr)
            .await
            .map_or_else(|v| error!("failed fix running_status, {v:?}"), |n| n);

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

pub async fn instance_health_check(state: AppState) {
    let is_master = Arc::new(RwLock::new(false));
    let state_clone = state.clone();
    let is_master_clone = is_master.clone();
    tokio::spawn(async move {
        let mut l = LeaderElection::new(state_clone.redis(), "jiascheduler:leader_election", 10)
            .expect("failed initialize leader election");

        l.run_election(|ok| {
            let is_master_clone = is_master_clone.clone();
            Box::pin(async move {
                info!("got leader election result {ok}");
                let mut val = is_master_clone.write().await;
                *val = ok;
                ()
            })
        })
        .await
        .expect("faild run leader election");
    });

    tokio::spawn(async move {
        let svc = state.service();
        loop {
            let ok = is_master.read().await;
            if *ok {
                info!("start offline inactive instance");
                let _ = svc
                    .instance
                    .offline_inactive_instance(60)
                    .await
                    .context("failed offline inactive instance")
                    .map_err(|e| error!("{e:?}"));

                sleep(Duration::from_secs(30)).await;
            } else {
                sleep(Duration::from_secs(1)).await;
            }
        }
    });
}

pub async fn start(state: AppState) -> Result<()> {
    let bus = Bus::new(state.redis().clone());

    instance_health_check(state.clone()).await;

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
