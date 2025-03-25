use crate::config::Conf;
use crate::logic::role;
use crate::logic::ssh::SshLogic;
use crate::logic::tag::TagLogic;
use crate::logic::team::TeamLogic;
use crate::logic::types::Permission;
use crate::logic::{
    executor::ExecutorLogic, instance::InstanceLogic, job::JobLogic, migration::MigrationLogic,
    role::RoleLogic, user::UserLogic,
};

use anyhow::{Ok, Result};
use casbin::{CoreApi, EnforceArgs, Enforcer, MgmtApi, RbacApi};

use redis::Client;
use rustc_serialize::hex::{FromHex, ToHex};
use sea_orm::DatabaseConnection;
use simple_crypt::{decrypt, encrypt};
use tokio::time::Instant;

use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

pub struct Service<'a> {
    pub user: UserLogic<'a>,
    pub job: JobLogic<'a>,
    pub executor: ExecutorLogic<'a>,
    pub instance: InstanceLogic<'a>,
    pub migration: MigrationLogic<'a>,
    pub role: RoleLogic<'a>,
    pub ssh: SshLogic<'a>,
    pub team: TeamLogic<'a>,
    pub tag: TagLogic<'a>,
}

#[derive(Clone)]
pub enum AppState {
    Inner(AppContext),
    Uninitialized,
}

impl Deref for AppState {
    type Target = AppContext;

    fn deref(&self) -> &Self::Target {
        match self {
            AppState::Inner(app_state) => app_state,
            AppState::Uninitialized => unreachable!(),
        }
    }
}

impl DerefMut for AppState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            AppState::Inner(app_state) => app_state,
            AppState::Uninitialized => unreachable!(),
        }
    }
}

pub struct AppContextBuilder {
    db: Option<DatabaseConnection>,
    redis: Option<Client>,
    conf: Option<Conf>,
    http_client: Option<reqwest::Client>,
    enforcer: Option<Arc<RwLock<Enforcer>>>,
    rate_limiter: Option<RateLimiter>,
}

impl AppContextBuilder {
    pub fn db(mut self, db: DatabaseConnection) -> Self {
        self.db = Some(db);
        self
    }

    pub fn redis(mut self, redis: Client) -> Self {
        self.redis = Some(redis);
        self
    }

    pub fn conf(mut self, conf: Conf) -> Self {
        self.conf = Some(conf);
        self
    }
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }
    pub fn rate_limit(mut self, secs: u64) -> Self {
        self.rate_limiter = Some(RateLimiter::new(secs));
        self
    }

    pub fn enforcer(mut self, mut enforcer: Enforcer) -> Self {
        enforcer.enable_auto_build_role_links(true);
        self.enforcer = Some(Arc::new(RwLock::new(enforcer)));
        self
    }

    pub fn build(self) -> Result<AppContext> {
        Ok(AppContext {
            db: self
                .db
                .ok_or(anyhow::anyhow!("database connection is required"))?,
            redis: self
                .redis
                .ok_or(anyhow::anyhow!("redis client is required"))?,
            conf: self.conf.ok_or(anyhow::anyhow!("config is required"))?,
            http_client: self
                .http_client
                .ok_or(anyhow::anyhow!("http client is required"))?,
            enforcer: self
                .enforcer
                .ok_or(anyhow::anyhow!("enforcer is required"))?,
            rate_limiter: Arc::new(RwLock::new(
                self.rate_limiter
                    .ok_or(anyhow::anyhow!("rate limiter is required"))?,
            )),
        })
    }
}

pub struct RateLimiter {
    last_executed: Instant,
    interval: Duration,
}

impl RateLimiter {
    pub fn new(interval_secs: u64) -> Self {
        RateLimiter {
            last_executed: Instant::now() - Duration::from_secs(interval_secs),
            interval: Duration::from_secs(interval_secs),
        }
    }

    pub fn can_execute(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_executed) >= self.interval {
            self.last_executed = now;
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct AppContext {
    pub db: DatabaseConnection,
    redis: Client,
    pub conf: Conf,
    rate_limiter: Arc<RwLock<RateLimiter>>,
    pub http_client: reqwest::Client,
    pub enforcer: Arc<RwLock<Enforcer>>,
}

impl AppContext {
    pub fn builder() -> AppContextBuilder {
        AppContextBuilder {
            enforcer: None,
            db: None,
            redis: None,
            conf: None,
            http_client: None,
            rate_limiter: None,
        }
    }

    pub fn service(&self) -> Service {
        Service {
            user: UserLogic::new(self),
            job: JobLogic::new(self),
            instance: InstanceLogic::new(self),
            executor: ExecutorLogic::new(self),
            role: RoleLogic::new(self),
            migration: MigrationLogic::new(self),
            ssh: SshLogic::new(self),
            team: TeamLogic::new(self),
            tag: TagLogic::new(self),
        }
    }

    pub fn redis(&self) -> Client {
        self.redis.clone()
    }

    pub async fn can_execute(&mut self) -> bool {
        let mut limiter = self.rate_limiter.write().await;
        limiter.can_execute()
    }

    pub fn encrypt(&self, data: String) -> Result<String> {
        let key = self.conf.encrypt.private_key.as_bytes();
        let b = encrypt(data.as_bytes(), key)?;
        let output = b.to_hex();
        Ok(output)
    }

    pub fn decrypt(&self, encrypt_data: String) -> Result<String> {
        let key = self.conf.encrypt.private_key.as_bytes();
        let data = encrypt_data.from_hex()?;
        let b = decrypt(data.as_slice(), key)?;
        Ok(String::from_utf8_lossy(&b).to_string())
    }

    pub async fn get_permissions_for_user(&self, user: &str) -> Result<Vec<String>> {
        let mut e = self.enforcer.write().await;
        let acts = e
            .get_implicit_permissions_for_user(user, None)
            .into_iter()
            .map(|v| v[1..].join("_").to_string())
            .collect::<Vec<String>>();

        Ok(acts)
    }

    pub async fn get_permissions_for_role(&self, role_id: u64) -> Result<Vec<String>> {
        let e = self.enforcer.read().await;
        let acts = e
            .get_filtered_policy(0, vec![role_id.to_string()])
            .into_iter()
            .map(|v| v[1..].join("_").to_string())
            .collect::<Vec<String>>();

        Ok(acts)
    }

    pub async fn enforce<T: EnforceArgs>(&self, val: T) -> Result<bool> {
        let e = self.enforcer.read().await;
        Ok(e.enforce(val)?)
    }

    pub async fn delete_role_for_user(&self, user_id: &str) -> Result<()> {
        let mut e = self.enforcer.write().await;
        e.delete_roles_for_user(user_id.into(), None).await?;
        Ok(())
    }

    pub async fn delete_role(&self, role_id: u64) -> Result<()> {
        let mut e = self.enforcer.write().await;
        e.remove_filtered_policy(0, vec![role_id.to_string()])
            .await?;
        Ok(())
    }

    pub async fn set_role_for_user(&self, user_id: &str, role_id: &str) -> Result<()> {
        let mut e = self.enforcer.write().await;
        if !e.has_role_for_user(user_id.as_ref(), role_id.as_ref(), None) {
            e.delete_roles_for_user(user_id.into(), None).await?;
            e.add_role_for_user(user_id.into(), role_id.into(), None)
                .await?;
        }
        Ok(())
    }

    pub async fn load_policy(&self) -> Result<()> {
        let mut e = self.enforcer.write().await;
        e.load_policy().await?;
        Ok(())
    }

    pub async fn set_policy(&self, role_id: &str, object: &str, action: &str) -> Result<()> {
        let mut e = self.enforcer.write().await;
        let p = vec![role_id.into(), object.into(), action.into()];

        if !e.has_policy(p.clone()) {
            e.add_policy(p).await?;
        }
        Ok(())
    }

    pub async fn init_admin_permission(&self) -> Result<()> {
        self.set_permission_manage_instance("1").await?;
        self.set_permission_manage_user("1").await?;
        self.set_permission_manage_job("1").await?;
        self.load_policy().await?;
        Ok(())
    }

    pub async fn can_manage_instance(&self, user_id: &str) -> Result<bool> {
        Ok(self.enforce((user_id, "instance", "manage")).await?)
    }

    pub async fn can_manage_user(&self, user_id: &str) -> Result<bool> {
        Ok(self.enforce((user_id, "user", "manage")).await?)
    }

    pub async fn can_upload_file(&self, user_id: &str) -> Result<bool> {
        Ok(self.enforce((user_id, "file", "upload")).await?)
    }

    // can manage job so can manage team
    pub async fn can_manage_job(&self, user_id: &str) -> Result<bool> {
        Ok(self.enforce((user_id, "job", "manage")).await?)
    }
    pub async fn is_change_forbid(&self, user_id: &str) -> Result<bool> {
        Ok(self.enforce((user_id, "change", "forbid")).await?)
    }

    pub async fn check_permissions(&self, user_id: &str, val: Vec<&Permission>) -> Result<bool> {
        for p in val {
            let pass = self.enforce((user_id, p.object, p.action)).await?;
            if !pass {
                return Ok(pass);
            }
        }

        return Ok(true);
    }

    pub async fn set_permissions(&self, role_id: u64, keys: Vec<String>) -> Result<()> {
        self.delete_role(role_id).await?;
        for key in keys {
            let pair = key.splitn(2, "_").collect::<Vec<&str>>();
            let (object, action) = (pair[0], pair[1]);

            let got = role::PERMISSIONS
                .iter()
                .find(|&v| v.object == object && v.action == action);
            if got.is_none() {
                anyhow::bail!("invalid permission key")
            }
            self.set_policy(role_id.to_string().as_str(), object, action)
                .await?;
        }
        self.load_policy().await?;
        Ok(())
    }

    pub async fn set_permission_manage_job(&self, role: &str) -> Result<()> {
        self.set_policy(role, "job", "manage").await
    }

    pub async fn set_permission_manage_user(&self, role: &str) -> Result<()> {
        self.set_policy(role, "user", "manage").await
    }

    pub async fn set_permission_manage_instance(&self, role: &str) -> Result<()> {
        self.set_policy(role, "instance", "manage").await
    }

    pub async fn set_permission_forbid_change(&self, role: &str) -> Result<()> {
        self.set_policy(role, "change", "forbid").await
    }
}

#[test]
fn crypt_test() {
    let data = "hello world".to_string();
    let key = "hello";

    let b = encrypt(data.as_bytes(), key.as_bytes()).unwrap();
    let output = b.to_hex();

    let b = output.from_hex().unwrap();
    let dec_val = decrypt(&b, key.as_bytes()).unwrap();

    let dec_val = String::from_utf8_lossy(&dec_val).to_string();
    assert_eq!(data, dec_val);
}
