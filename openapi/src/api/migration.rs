use anyhow::Context;
use migration::MigratorTrait;
use nanoid::nanoid;
use poem::{session::Session, web::Data, Result};
use poem_openapi::{param::Query, payload::Json, OpenApi};
use redis::Client;
use sea_orm::{ConnectOptions, Database};
use tokio::sync::mpsc::Sender;
use url::Url;

use crate::{
    api_response,
    config::Conf,
    logic::{self, user::UserLogic},
    response::{anyhow_into_error, std_into_error, ApiStdResponse},
    return_err, return_ok, AppState, InstallState,
};

mod types {
    use poem_openapi::Object;
    use serde::{Deserialize, Serialize};

    #[derive(Object, Serialize, Deserialize)]
    pub struct UpgradeVersionReq {
        pub version: String,
    }

    #[derive(Object, Serialize, Deserialize)]
    pub struct UpgradeVersionResp {
        pub result: u64,
    }

    #[derive(Object, Serialize, Default)]
    pub struct QueryVersionResp {
        pub total: u64,
        pub list: Vec<VersionRecord>,
    }

    #[derive(Object, Serialize, Default, Clone)]
    pub struct VersionRecord {
        pub name: String,
        pub info: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct GetDatabaseResp {
        pub name: String,
        pub sql: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct InstallResp {
        pub result: i32,
    }

    fn default_up() -> String {
        "up".to_string()
    }

    #[derive(Object, Serialize, Default)]
    pub struct InstallReq {
        #[oai(validator(min_length = 1, max_length = 50))]
        pub username: String,
        #[oai(validator(min_length = 1, max_length = 50))]
        pub password: String,
        pub database_url: String,
        pub redis_url: String,
        pub bind_addr: String,
        pub comet_secret: String,
        #[oai(default = "default_up")]
        pub migration_type: String,
    }

    #[derive(Object, Serialize, Default)]
    pub struct CheckVersionResp {
        pub config_file: Option<String>,
        pub is_installed: bool,
        pub current_version: String,
        pub bind_addr: String,
        pub need_upgrade: bool,
    }
}

pub struct MigrationApi;

#[OpenApi(prefix_path = "/migration", tag = super::Tag::Migration)]
impl MigrationApi {
    #[oai(path = "/version/upgrade", method = "post")]
    pub async fn upgrade_version(
        &self,
        _user_info: Data<&logic::types::UserInfo>,
        _session: &Session,
        state: Data<&AppState>,
        Json(req): Json<types::UpgradeVersionReq>,
    ) -> Result<ApiStdResponse<types::UpgradeVersionResp>> {
        let svc = state.service();
        let ret = svc.migration.migrate(&req.version).await?;
        return_ok!(types::UpgradeVersionResp { result: ret })
    }

    #[oai(path = "/version/list", method = "get")]
    pub async fn query_version(
        &self,
        _user_info: Data<&logic::types::UserInfo>,
        _session: &Session,

        Query(name): Query<Option<String>>,
        #[oai(
            default = "crate::api::default_page_size",
            validator(maximum(value = "10000"))
        )]
        Query(page_size): Query<u64>,
        #[oai(
            default = "crate::api::default_page",
            validator(maximum(value = "10000"))
        )]
        Query(page): Query<u64>,
        state: Data<&AppState>,
    ) -> api_response!(types::QueryVersionResp) {
        let svc = state.service();
        let ret = svc.migration.query_version(name, page, page_size)?;
        let list = ret
            .0
            .into_iter()
            .map(|v| types::VersionRecord {
                name: v.name,
                info: v.info,
            })
            .collect();

        return_ok!(types::QueryVersionResp { total: ret.1, list })
    }

    #[oai(path = "/database/get", method = "get")]
    pub async fn get_database(
        &self,
        _user_info: Data<&logic::types::UserInfo>,
        _session: &Session,
        Query(name): Query<String>,
        state: Data<&AppState>,
    ) -> api_response!(types::GetDatabaseResp) {
        let svc = state.service();
        let ret = svc.migration.get_database(&name).await?;

        match ret {
            Some(v) => return_ok!(types::GetDatabaseResp {
                name: v.0,
                sql: v.1,
            }),
            None => return_err!("not found"),
        }
    }

    #[oai(path = "/version/check", method = "get")]
    pub async fn check_version(
        &self,
        install_state: Data<&InstallState>,
        state: Data<&AppState>,
    ) -> Result<ApiStdResponse<types::CheckVersionResp>> {
        let need_upgrade = if install_state.is_installed {
            !migration::Migrator::get_pending_migrations(&state.db)
                .await
                .map_err(std_into_error)?
                .is_empty()
        } else {
            false
        };

        return_ok!(types::CheckVersionResp {
            is_installed: install_state.is_installed,
            current_version: install_state.current_version.clone(),
            bind_addr: install_state.bind_addr.clone(),
            config_file: install_state.config_file.clone(),
            need_upgrade
        })
    }

    #[oai(path = "/install", method = "post")]
    pub async fn install(
        &self,
        install_state: Data<&InstallState>,
        Json(req): Json<types::InstallReq>,
        tx: Data<&Sender<()>>,
    ) -> Result<ApiStdResponse<types::InstallResp>> {
        // 1. connect database
        let database_url = Url::parse(&req.database_url)
            .context("database url")
            .map_err(anyhow_into_error)?;

        let opt = ConnectOptions::new(database_url);
        let conn = Database::connect(opt).await.map_err(std_into_error)?;

        // 2. connect redis
        let redis_url = Url::parse(&req.redis_url)
            .context("redis url")
            .map_err(anyhow_into_error)?;
        Client::open(redis_url)
            .context("connect redis")
            .map_err(anyhow_into_error)?;

        if req.migration_type == "up" {
            migration::Migrator::up(&conn, None)
                .await
                .map_err(std_into_error)?;
        }

        // 2. create admin user
        let _ = UserLogic::init_admin(&conn, &req.username, &req.password).await?;

        // 3. generate config file
        let mut conf = Conf::default();
        conf.database_url = req.database_url;
        conf.redis_url = req.redis_url;
        conf.bind_addr = req.bind_addr;
        conf.admin.username = req.username;
        conf.admin.password = req.password;
        conf.comet_secret = req.comet_secret;
        conf.encrypt.private_key = nanoid!();
        conf.sync2file(install_state.config_file.clone())?;

        tx.send(()).await.map_err(|v| std_into_error(v))?;
        return_ok!(types::InstallResp { result: 0 })
    }
}