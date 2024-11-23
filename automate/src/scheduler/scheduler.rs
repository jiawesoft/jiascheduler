use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, Utc};
use futures::{SinkExt, StreamExt};

use crate::{
    bridge::msg::{
        BundleOutputParams, RuntimeActionParams, SftpDownloadParams, SftpReadDirParams,
        SftpRemoveParams, SftpUploadParams, UpdateJobParams,
    },
    comet::types::SshLoginParams,
    get_comet_addr, get_local_ip, get_mac_address,
    scheduler::types::JobAction,
    set_comet_addr,
    ssh::{self, ConnectParams, Session},
};
use futures_util::stream::{SplitSink, SplitStream};

use serde_json::{json, Value};
use tokio::{
    net::TcpStream,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
    task,
    time::{sleep, timeout},
};
use tokio_cron_scheduler::{Job, JobScheduler};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{ClientRequestBuilder, Message},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info};
use uuid::Uuid;

use super::{
    executor::Ctx,
    file::try_download_file,
    types::{
        self, AssignUserOption, BundleOutput, RuntimeAction, ScheduleType, SshConnectionOption,
    },
};

use crate::{
    bridge::{
        client::WsClient,
        msg::{DispatchJobParams, HeartbeatParams, MsgReqKind},
        Bridge,
    },
    get_endpoint,
    scheduler::executor::Executor,
};

#[derive(Clone)]
pub struct React {
    sched: JobScheduler,
    bridge: Bridge,
    output_dir: String,
    namespace: String,
    local_ip: String,
    client_key: String,
    schedule_uuid_mapping: Arc<Mutex<HashMap<String, Uuid>>>,
    kill_signal_mapping: Arc<Mutex<HashMap<String, Vec<Sender<()>>>>>,
}

impl React {
    async fn new(
        bridge: Bridge,
        namespace: String,
        local_ip: String,
        client_key: String,
        output_dir: String,
    ) -> Self {
        Self {
            sched: JobScheduler::new().await.unwrap(),
            output_dir,
            schedule_uuid_mapping: Arc::new(Mutex::new(HashMap::new())),
            kill_signal_mapping: Arc::new(Mutex::new(HashMap::new())),
            bridge,
            client_key,
            namespace,
            local_ip,
        }
    }

    async fn send_update_job_msg(&self, data: UpdateJobParams) -> Result<Value> {
        self.send_bridge_msg(MsgReqKind::UpdateJobRequest(data))
            .await
    }

    async fn send_bridge_msg(&self, data: MsgReqKind) -> Result<Value> {
        self.bridge.send_msg(&self.client_key, data).await
    }

    async fn add_job_schedule(
        &mut self,
        job_id: String,
        job: Job,
    ) -> Result<Option<DateTime<Utc>>> {
        self.remove_job_schedule(job_id.as_str()).await?;

        let mut locked_map = self.schedule_uuid_mapping.lock().await;
        if locked_map.get(&job_id).is_some() {
            anyhow::bail!("{job_id} is existed");
        }

        let uuid = self.sched.add(job).await?;

        let next_time = self.sched.next_tick_for_job(uuid).await?;

        locked_map.insert(job_id, uuid);
        Ok(next_time)
    }

    async fn remove_job_schedule(&mut self, job_id: &str) -> Result<()> {
        let mut locked_map = self.schedule_uuid_mapping.lock().await;
        if let Some(uuid) = locked_map.get(job_id) {
            self.sched.remove(uuid).await?;
            locked_map.remove(job_id);
        }
        Ok(())
    }

    async fn add_kill_signal_tx(&mut self, job_id: String, kill_signal_tx: Sender<()>) {
        let mut locked_map = self.kill_signal_mapping.lock().await;
        if let Some(val) = locked_map.get_mut(&job_id) {
            val.append(&mut vec![kill_signal_tx]);
        } else {
            locked_map.insert(job_id, vec![kill_signal_tx]);
        }
    }

    async fn kill_job(&mut self, job_id: String) {
        let mut locked_map = self.kill_signal_mapping.lock().await;
        locked_map.remove(&job_id).map(|v| async {
            for tx in v {
                if let Err(_) = tx.send(()).await {
                    error!("failed send kill signal, job_id: {job_id}");
                }
            }
        });
    }

    async fn remove_job(&mut self, job_id: String) {
        let mut locked_map = self.kill_signal_mapping.lock().await;
        locked_map.remove(&job_id);
    }

    async fn start(&mut self) -> Result<()> {
        self.sched.start().await?;
        Ok(())
    }
}

pub struct Scheduler<T> {
    comet_addr: Vec<String>,
    comet_secret: String,
    mac_address: String,
    output_dir: String,
    is_initialized: bool,
    client: Option<T>,
    pub namespace: String,
    bridge: Bridge,
    ssh_connection_option: Option<SshConnectionOption>,
    assign_user_option: Option<AssignUserOption>,
}

impl
    Scheduler<
        WsClient<
            SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
            SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        >,
    >
{
    pub fn new(
        namespace: String,
        comet_addr: Vec<String>,
        comet_secret: String,
        output_dir: String,
        ssh_connection_option: Option<SshConnectionOption>,
        assign_user_option: Option<AssignUserOption>,
    ) -> Self {
        Scheduler {
            comet_addr,
            comet_secret,
            output_dir,
            client: None,
            mac_address: get_mac_address().expect("failed get mac address"),
            is_initialized: false,
            namespace,
            bridge: Bridge::new(),
            ssh_connection_option,
            assign_user_option,
        }
    }

    pub fn client_key(&self) -> String {
        get_endpoint(self.namespace.clone(), get_local_ip().to_string())
    }

    pub fn get_comet_addr(&mut self) -> String {
        if let Some(v) = self.comet_addr.pop() {
            self.comet_addr.push(v.clone());
            return v;
        }
        panic!("comet_addr is empty");
    }

    pub async fn ssh_poll(&mut self) {
        let comet_secret = self.comet_secret.clone();
        let namespace = self.namespace.clone();

        tokio::spawn(async move {
            loop {
                let addr = if let Some(addr) = get_comet_addr() {
                    addr
                } else {
                    sleep(Duration::from_millis(100)).await;
                    continue;
                };

                if let Err(e) =
                    Self::ssh_keepalive(namespace.clone(), addr.clone(), comet_secret.clone()).await
                {
                    error!("failed ssh keepalive {e}");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        });
    }

    pub async fn ssh_keepalive(
        namespace: String,
        addr: String,
        comet_secret: String,
    ) -> anyhow::Result<()> {
        let local_ip = get_local_ip();
        let endpoint = get_endpoint(namespace, local_ip.to_string());
        info!("current_point {}", endpoint);
        let addr = format!("{}/ssh/register/{}", addr, endpoint);
        let u = addr.parse::<poem::http::Uri>()?;
        let req = ClientRequestBuilder::new(u)
            .with_header("Authorization", format!("Bearer {}", comet_secret));
        let (ws_stream, _b) = connect_async(req).await?;
        let (mut sink, mut stream) = ws_stream.split();

        let login_params = loop {
            let next_stream = match timeout(Duration::from_secs(60), stream.next()).await {
                Ok(v) => v,
                Err(e) => {
                    debug!("timeout {e}, retry!");
                    return Ok(());
                }
            };

            match next_stream {
                Some(ret) => {
                    let msg = ret?;
                    if let Message::Text(ready) = msg {
                        let val: SshLoginParams = serde_json::from_str(&ready)?;
                        break val;
                    } else {
                        return Ok(());
                    }
                }
                _ => return Ok(()),
            };
        };

        tokio::spawn(async move {
            let sess = match Session::connect(ConnectParams {
                user: login_params.user,
                password: login_params.password,
                addrs: (local_ip, login_params.port),
            })
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    let _ = sink
                        .send(Message::Text(format!(
                            "\r\n\x1b[31mNotice: failed connect to target server, {e}"
                        )))
                        .await
                        .map_err(|e| error!("failed send message to ws connection - {e}"));
                    return;
                }
            };

            let code = match sess.call2("bash", 20, 30, &mut sink, stream).await {
                Ok(v) => v,
                Err(e) => {
                    let _ = sink
                        .send(Message::Text(format!(
                            "\r\n\x1b[31mNotice: connection closed - {e}"
                        )))
                        .await
                        .map_err(|e| error!("failed send message to ws connection - {e}"));
                    return;
                }
            };

            info!("web ssh exit code {code}");

            if let Err(e) = sess.close().await {
                error!("failed close - {e}");
            }
            info!("ssh tunnel close");
        });

        Ok(())
    }

    pub async fn connect_comet(&mut self) -> anyhow::Result<()> {
        let addr = self.get_comet_addr();
        let local_ip = get_local_ip();

        let mut client = WsClient::new(Some(self.bridge.clone()))
            .set_namespace(self.namespace.clone())
            .set_local_ip(local_ip.clone())
            .set_comet_secret(self.comet_secret.clone())
            .set_mac_address(self.mac_address.clone());

        if let Some(ref opt) = self.assign_user_option {
            client = client.set_assign_user(opt.to_owned());
        }

        if let Some(ref opt) = self.ssh_connection_option {
            client = client.set_ssh_connection(opt.to_owned());
        }

        let ws_addr = format!("{}/evt/{}", addr, self.namespace);

        client.connect(&ws_addr, &self.comet_secret).await?;
        let client_key = self.client_key();

        set_comet_addr(addr);

        info!("append new sender {client_key} to {ws_addr}");

        self.bridge
            .append_client(client_key.clone(), client.sender())
            .await;

        self.client.replace(client);
        self.is_initialized = true;
        Ok(())
    }

    async fn exec_job(
        e: Executor,
        react: React,
        schedule_type: Option<ScheduleType>,
        kill_signal_rx: Receiver<()>,
        prev_time: Option<DateTime<Utc>>,
        next_time: Option<DateTime<Utc>>,
        job_params: DispatchJobParams,
    ) -> Result<BundleOutput> {
        let start_time = Utc::now();
        let schedule_id = job_params.schedule_id;
        let base_job = job_params.base_job;

        let _ = react
            .send_update_job_msg(UpdateJobParams {
                base_job: base_job.to_pure_job(),
                run_status: Some(types::RunStatus::Running),
                schedule_id: schedule_id.clone(),
                exit_status: None,
                exit_code: None,
                stdout: None,
                stderr: None,
                next_time,
                prev_time,
                bind_namespace: react.namespace.clone(),
                bind_ip: react.local_ip.clone(),
                schedule_type: schedule_type.clone(),
                created_user: job_params.created_user.clone(),
                start_time: Some(start_time.clone()),
                ..Default::default()
            })
            .await?;

        let output = match e.run(Ctx { kill_signal_rx }).await {
            Ok(v) => v,
            Err(e) => {
                let bundle_output = if base_job.bundle_script.is_none() {
                    None
                } else {
                    Some(vec![])
                };
                let _ = react
                    .send_update_job_msg(UpdateJobParams {
                        base_job: base_job.to_pure_job(),
                        run_status: Some(types::RunStatus::Stop),
                        schedule_id: schedule_id.clone(),
                        exit_status: Some(e.to_string()),
                        exit_code: Some(99),
                        prev_time,
                        next_time,
                        bind_namespace: react.namespace.clone(),
                        bind_ip: react.local_ip.clone(),
                        start_time: Some(start_time),
                        schedule_type: schedule_type.clone(),
                        stdout: Some(e.to_string()),
                        stderr: Some(e.to_string()),
                        end_time: Some(Utc::now()),
                        created_user: job_params.created_user.clone(),
                        bundle_output,
                        ..Default::default()
                    })
                    .await?;
                return Err(e);
            }
        };

        let _ = react
            .send_update_job_msg(UpdateJobParams {
                base_job: base_job.to_pure_job(),
                run_status: Some(types::RunStatus::Stop),
                schedule_id: schedule_id.clone(),
                exit_status: output.get_exit_status(),
                exit_code: output.get_exit_code(),
                prev_time,
                next_time,
                bind_namespace: react.namespace.clone(),
                bind_ip: react.local_ip.clone(),
                start_time: Some(start_time),
                schedule_type: schedule_type.clone(),
                stdout: output.get_stdout(),
                stderr: output.get_stderr(),
                end_time: Some(Utc::now()),
                created_user: job_params.created_user.clone(),
                bundle_output: BundleOutputParams::parse(&output),
                ..Default::default()
            })
            .await?;

        Ok(output)
    }

    async fn start_timer(dispatch_params: DispatchJobParams, mut react: React) -> Result<Value> {
        let timer_expr = dispatch_params.timer_expr.clone().unwrap_or_default();
        let base_job = dispatch_params.base_job.clone();
        let pure_job = base_job.to_pure_job();
        let euid = dispatch_params.base_job.eid.clone();
        let react_clone = react.clone();
        let created_user = dispatch_params.created_user.clone();
        let schedule_id = dispatch_params.schedule_id.clone();

        let job = Job::new_cron_job_async_tz(
            timer_expr.as_str(),
            Local,
            move |job_id, mut job_scheduler| {
                let base_job = base_job.clone();
                let (kill_signal_tx, kill_signal_rx) = channel::<()>(1);
                let mut react_clone = react_clone.clone();
                let dispatch_params = dispatch_params.clone();

                Box::pin(async move {
                    let next_time = job_scheduler.next_tick_for_job(job_id).await.unwrap();
                    let prev_time = Some(Local::now().into());

                    react_clone
                        .add_kill_signal_tx(base_job.eid.clone(), kill_signal_tx)
                        .await;

                    let e = Executor::builder()
                        .job(base_job.clone())
                        .output_dir(react_clone.output_dir.clone())
                        .disable_write_log(true)
                        .build();
                    match Self::exec_job(
                        e,
                        react_clone,
                        Some(ScheduleType::Timer),
                        kill_signal_rx,
                        prev_time,
                        next_time,
                        dispatch_params,
                    )
                    .await
                    {
                        Ok(_) => {}
                        Err(e) => error!("failed exec {} - detail: {e}", base_job.eid),
                    }
                })
            },
        )
        .map_err(|v| anyhow!("failed parse timer expr {} - {}", timer_expr, v))?;

        let next_time = react.add_job_schedule(euid, job).await?;

        let _ = react
            .send_update_job_msg(UpdateJobParams {
                base_job: pure_job,
                run_status: Some(types::RunStatus::Prepare),
                schedule_status: Some(types::ScheduleStatus::Scheduling),
                schedule_id,
                exit_status: None,
                exit_code: None,
                stdout: None,
                stderr: None,
                next_time,
                bind_namespace: react.namespace.clone(),
                bind_ip: react.local_ip.clone(),
                schedule_type: Some(ScheduleType::Timer),
                created_user,
                start_time: None,
                ..Default::default()
            })
            .await?;

        Ok(json!(null))
    }

    async fn stop_timer(dispatch_params: DispatchJobParams, mut react: React) -> Result<Value> {
        react
            .remove_job_schedule(&dispatch_params.base_job.eid)
            .await?;
        let _ = react
            .send_update_job_msg(UpdateJobParams {
                base_job: dispatch_params.base_job.to_pure_job(),
                schedule_status: Some(types::ScheduleStatus::Unscheduled),
                run_status: None,
                schedule_id: dispatch_params.schedule_id,
                exit_status: None,
                exit_code: None,
                stdout: None,
                stderr: None,
                next_time: None,
                bind_namespace: react.namespace.clone(),
                bind_ip: react.local_ip.clone(),
                schedule_type: Some(ScheduleType::Timer),
                created_user: dispatch_params.created_user,
                start_time: None,
                ..Default::default()
            })
            .await?;
        Ok(json!(null))
    }

    async fn exec(dispatch_params: DispatchJobParams, mut react: React) -> Result<Value> {
        let base_job = dispatch_params.base_job.clone();
        let (kill_signal_tx, kill_signal_rx) = channel::<()>(1);

        let e = Executor::builder()
            .job(base_job.clone())
            .output_dir(react.output_dir.clone())
            .disable_write_log(true)
            .build();

        react
            .add_kill_signal_tx(base_job.eid.clone(), kill_signal_tx)
            .await;

        if dispatch_params.is_sync {
            let output = Self::exec_job(
                e,
                react.clone(),
                Some(ScheduleType::Once),
                kill_signal_rx,
                None,
                None,
                dispatch_params,
            )
            .await?;
            react.remove_job(base_job.eid.clone()).await;
            return Ok(json!({
                "stdout":output.get_stdout(),
                "exit_code":output.get_exit_code(),
                "stderr":output.get_stderr(),
            }));
        }
        let juid = base_job.eid.clone();

        task::spawn(async move {
            match Self::exec_job(
                e,
                react.clone(),
                Some(ScheduleType::Once),
                kill_signal_rx,
                None,
                None,
                dispatch_params,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => error!("failed exec {} - detail: {e}", base_job.eid),
            }

            react.remove_job(juid).await
        });

        return Ok(json!(null));
    }

    async fn kill(dispatch_params: DispatchJobParams, mut react: React) -> Result<Value> {
        react.kill_job(dispatch_params.base_job.eid.clone()).await;
        Ok(json!(null))
    }

    pub async fn dispath_job(dispatch_params: DispatchJobParams, react: React) -> Result<Value> {
        let mut base_job = dispatch_params.base_job.clone();
        let upload_file = base_job.upload_file.take();

        if matches!(
            dispatch_params.action,
            JobAction::StartTimer | JobAction::Exec
        ) {
            if let Some(comet_addr) = get_comet_addr() {
                try_download_file(comet_addr, upload_file).await?;
            }
        }

        match dispatch_params.action {
            JobAction::StartTimer => Scheduler::start_timer(dispatch_params, react).await,
            JobAction::StopTimer => Scheduler::stop_timer(dispatch_params, react).await,
            JobAction::StartSupervisor => todo!(),
            JobAction::StopSupervisor => todo!(),
            JobAction::Exec => Scheduler::exec(dispatch_params, react).await,
            JobAction::Kill => Scheduler::kill(dispatch_params, react).await,
        }
    }

    pub async fn runtime_action(
        action_params: RuntimeActionParams,
        mut react: React,
    ) -> Result<Value> {
        match action_params.action {
            RuntimeAction::StopTimer => react.remove_job_schedule(&action_params.eid).await?,
            RuntimeAction::StopSupervisor => todo!(),
            RuntimeAction::Kill => react.kill_job(action_params.eid).await,
        };
        Ok(json!(null))
    }

    pub async fn sftp_read_dir(req: SftpReadDirParams) -> Result<Value> {
        let ret = ssh::read_dir(
            &req.ip,
            req.port,
            &req.user,
            &req.password,
            req.dir.filter(|v| v != "").as_deref(),
        )
        .await?;
        let ret = serde_json::to_value(ret)?;
        Ok(ret)
    }

    pub async fn sftp_upload(req: SftpUploadParams) -> Result<Value> {
        let ret = ssh::upload(
            &req.ip,
            req.port,
            &req.user,
            &req.password,
            &req.filepath,
            req.data,
        )
        .await?;
        let ret = serde_json::to_value(ret)?;
        Ok(ret)
    }

    pub async fn sftp_download(req: SftpDownloadParams) -> Result<Value> {
        let ret = ssh::download(&req.ip, req.port, &req.user, &req.password, &req.filepath).await?;
        let ret = serde_json::to_value(ret)?;
        Ok(ret)
    }

    pub async fn sftp_remove(req: SftpRemoveParams) -> Result<Value> {
        let ret = ssh::remove(
            &req.ip,
            req.port,
            &req.user,
            &req.password,
            &req.remove_type,
            &req.filepath,
        )
        .await?;
        let ret = serde_json::to_value(ret)?;
        Ok(ret)
    }

    pub async fn handle(msg: MsgReqKind, _bridge: Bridge, react: React) -> Value {
        let ret = match msg {
            MsgReqKind::DispatchJobRequest(v) => Self::dispath_job(v, react.clone()).await,
            MsgReqKind::RuntimeActionRequest(v) => Self::runtime_action(v, react.clone()).await,
            MsgReqKind::SftpReadDirRequest(v) => Self::sftp_read_dir(v).await,
            MsgReqKind::SftpUploadRequest(v) => Self::sftp_upload(v).await,
            MsgReqKind::SftpRemoveRequest(v) => Self::sftp_remove(v).await,
            MsgReqKind::SftpDownloadRequest(v) => Self::sftp_download(v).await,
            MsgReqKind::PullJobRequest(_) => todo!(),
            MsgReqKind::HeartbeatRequest(_) => todo!(),
            _ => todo!(),
        };

        match ret {
            Ok(v) => {
                json!({
                    "code": 20000,
                    "msg": "success",
                    "data": v,
                })
            }
            Err(e) => json!({
                "code":50000,
                "msg":e.to_string(),
            }),
        }
    }

    pub async fn recv(&mut self, react: React) {
        let bridge = self.bridge.clone();

        while let Some(mut client) = self.client.take() {
            let bridge = bridge.clone();
            let react = react.clone();

            client
                .recv(|msg| async move { Self::handle(msg, bridge, react).await })
                .await;
            client.drop().await;
        }
    }

    pub async fn heartbeat(&self) {
        let bridge = self.bridge.clone();
        let client_key = self.client_key();
        let namespace = self.namespace.clone();
        let source_ip = get_local_ip().to_string();
        let mac_addr = self.mac_address.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(5)).await;
                match bridge
                    .send_msg(
                        &client_key,
                        MsgReqKind::HeartbeatRequest(HeartbeatParams {
                            namespace: namespace.clone(),
                            mac_addr: mac_addr.clone(),
                            source_ip: source_ip.clone(),
                        }),
                    )
                    .await
                {
                    Ok(_v) => {}
                    Err(e) => error!("failed heartbeat {e}, client_key:{client_key}"),
                }
                debug!("heartbeat!")
            }
        });
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let react = React::new(
            self.bridge.clone(),
            self.namespace.clone(),
            get_local_ip().to_string(),
            self.client_key(),
            self.output_dir.clone(),
        )
        .await;
        let mut react_clone: React = react.clone();

        self.ssh_poll().await;

        tokio::spawn(async move {
            react_clone
                .start()
                .await
                .expect("failed start cron scheduler");
        });
        self.heartbeat().await;
        loop {
            self.recv(react.clone()).await;
            info!("reconnect after 1s");
            sleep(Duration::from_secs(1)).await;
            if let Err(e) = self.connect_comet().await {
                error!("failed reconnect to comet {:?} - {e}", self.comet_addr);
            }
        }
    }
}
