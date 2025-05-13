pub mod macros;
pub mod response;

use anyhow::{anyhow, Context, Result};
use api::{
    executor::ExecutorApi, file::FileApi, instance::InstanceApi, job::JobApi, manage::ManageApi,
    migration::MigrationApi, role::RoleApi, tag::TagApi, team::TeamApi, terminal, user::UserApi,
    workflow::WorkflowApi,
};
use casbin::{CoreApi, DefaultModel, Enforcer};

use ::migration::{Migrator, MigratorTrait};

use logic::user::UserLogic;
use middleware::AuthMiddleware;
use poem::{get, IntoEndpoint};
use service::config::Conf;

pub use error::custom_error;

pub use openapi_derive::ApiStdResponse;
use poem::{
    endpoint::{EmbeddedFileEndpoint, EmbeddedFilesEndpoint},
    listener::TcpListener,
    session::{CookieConfig, RedisStorage, ServerSession},
    EndpointExt, Route,
};
use poem_openapi::{ContactObject, OpenApiService};
use redis::{aio::ConnectionManager, Client};

pub use entity;
use git_version::git_version;
use reqwest::header;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_adapter::SeaOrmAdapter;
use state::{AppContext, AppState};
use std::{path::Path, time::Duration};
use tokio::sync::{mpsc, oneshot::Sender};
use tracing::info;
use url::Url;

pub mod api;

mod error;
mod job;
pub use service::logic;
pub mod middleware;
mod migration;

pub use service::state;
pub mod utils;

#[derive(Clone)]
pub struct WebapiOptions {
    pub database_url: Option<String>,
    pub redis_url: Option<String>,
    pub bind_addr: Option<String>,
    pub config_file: String,
}

impl WebapiOptions {
    fn merge_conf(&self, config_path: &str) -> Result<Conf> {
        let real_path = shellexpand::full(config_path)?;
        let mut conf = Conf::parse(real_path.as_ref())?;

        let _ = self
            .database_url
            .iter()
            .map(|v| conf.database_url = v.to_string());
        let _ = self
            .redis_url
            .iter()
            .map(|v| conf.redis_url = v.to_string());
        let _ = self
            .bind_addr
            .iter()
            .map(|v| conf.bind_addr = v.to_string());

        Ok(conf)
    }
}

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../dist"]
pub struct Dist;

const RBAL_RESOURCE_ROLES_MODEL: &'static str = r#"
[request_definition]
r = sub, obj, act

[policy_definition]
p = sub, obj, act

[role_definition]
g = _, _
g2 = _, _

[policy_effect]
e = some(where (p.eft == allow))

[matchers]
m = g(r.sub, p.sub) && g2(r.obj, p.obj) && r.act == p.act
"#;

const GIT_VERSION: &str = git_version!();
const APP_VERSION: &str = "1.1.5";

fn get_version() -> String {
    format!("{APP_VERSION}-{GIT_VERSION}")
}

fn is_installed(config_file: &str) -> Result<bool> {
    let real_path = shellexpand::full(config_file)?;
    Ok(Path::new(&real_path.to_string()).exists())
}

#[derive(Clone)]
pub struct InstallState {
    current_version: String,
    is_installed: bool,
    bind_addr: String,
    config_file: Option<String>,
}

impl InstallState {
    pub fn new(is_installed: bool, bind_addr: String, config_file: Option<String>) -> Self {
        Self {
            current_version: get_version(),
            is_installed,
            bind_addr,
            config_file,
        }
    }
}

async fn install(opts: &WebapiOptions) -> Result<()> {
    let api_service =
        OpenApiService::new(MigrationApi, "jiascheduler web api", "1.0").server("/api");

    let (tx, mut rx) = mpsc::channel::<()>(1);

    let bind_addr = opts.bind_addr.clone().ok_or(anyhow!(
        "before initializing the installation, it is necessary to pass the bind_addr from command-line parameters"
    ))?;

    let app = Route::new()
        .at("/", EmbeddedFileEndpoint::<Dist>::new("index.html"))
        .nest("/", EmbeddedFilesEndpoint::<Dist>::new())
        .nest("/api", api_service.into_endpoint())
        .data(tx)
        .data(InstallState::new(
            false,
            bind_addr.clone(),
            Some(opts.config_file.to_string()),
        ))
        .data(AppState::Uninitialized)
        .catch_all_error(custom_error);

    poem::Server::new(TcpListener::bind(bind_addr))
        .run_with_graceful_shutdown(
            app,
            async move {
                rx.recv().await;
            },
            None,
        )
        .await?;
    Ok(())
}

pub async fn upgrade(conn: &DatabaseConnection) -> Result<()> {
    if !Migrator::get_pending_migrations(conn).await?.is_empty() {
        Migrator::up(conn, None).await?;
    }
    Ok(())
}

pub async fn run(opts: WebapiOptions, signal: Option<Sender<Conf>>) -> Result<()> {
    if !is_installed(&opts.config_file)? {
        info!("start initializing configuration file");
        install(&opts).await?;
        info!("complete initialization configuration file")
    }

    let conf = opts.merge_conf(&opts.config_file).context("merge config")?;
    let mut connect_opts =
        ConnectOptions::new(Url::parse(&conf.database_url).expect("database url"));
    connect_opts.sqlx_logging(false); // Disable SQLx log

    let conn = Database::connect(connect_opts.clone())
        .await
        .expect("failed connect to database");

    upgrade(&conn).await.context("upgrade version")?;

    UserLogic::init_admin(&conn, &conf.admin.username, &conf.admin.password)
        .await
        .context("failed initialize admin user")?;

    let client = Client::open(Url::parse(&conf.redis_url).expect("redis url")).unwrap();
    let m = DefaultModel::from_str(RBAL_RESOURCE_ROLES_MODEL)
        .await
        .expect("casbin model");
    let a: SeaOrmAdapter<DatabaseConnection> = SeaOrmAdapter::new(conn.clone())
        .await
        .expect("seaorm adapter");
    let e: Enforcer = Enforcer::new(m, a).await.unwrap();

    let mut headers = header::HeaderMap::new();

    let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {}", conf.comet_secret))?;
    auth_value.set_sensitive(true);
    headers.insert(header::AUTHORIZATION, auth_value);

    let ctx = AppContext::builder()
        .db(conn)
        .conf(conf.clone())
        .redis(client)
        .enforcer(e)
        .rate_limit(30)
        .http_client(
            reqwest::Client::builder()
                .default_headers(headers)
                .build()?,
        )
        .build()?;
    let state = AppState::Inner(ctx);

    let api_service = OpenApiService::new(
        (
            UserApi,
            TeamApi,
            JobApi,
            ExecutorApi,
            InstanceApi,
            FileApi,
            RoleApi,
            MigrationApi,
            ManageApi,
            TagApi,
            WorkflowApi,
        ),
        "jiascheduler web api",
        "1.0",
    )
    .summary("jiascheduler web api")
    .description(
        "A high-performance, scalable, dynamically configured job scheduler developed with rust",
    )
    .server("/api")
    .contact(
        ContactObject::new()
            .name("iwannay")
            .url("https://github.com/iwannay")
            .email("772648576@qq.com"),
    );

    state.service().user.load_user_role(&state).await?;
    state.init_admin_permission().await?;

    job::start(state.clone()).await?;

    let ui = api_service.rapidoc();
    let app = Route::new()
        .at("/", EmbeddedFileEndpoint::<Dist>::new("index.html"))
        .nest("/", EmbeddedFilesEndpoint::<Dist>::new())
        .at(
            "/terminal/webssh/:instance_id",
            get(terminal::webssh).with(AuthMiddleware),
        )
        .at(
            "/terminal/tunnel/:instance_id",
            get(terminal::proxy_webssh).with(AuthMiddleware),
        )
        .nest("/api", api_service.with(AuthMiddleware))
        .nest("/doc", ui)
        .catch_all_error(custom_error)
        .with(ServerSession::new(
            CookieConfig::default()
                .name("jiaschduler-sid")
                .max_age(Some(Duration::from_secs(86400)))
                .secure(false),
            RedisStorage::new(ConnectionManager::new(state.redis()).await.unwrap()),
        ))
        .data(state)
        .data(InstallState::new(
            true,
            conf.bind_addr.clone(),
            Some(opts.config_file),
        ));

    if let Some(tx) = signal {
        tx.send(conf.clone()).expect("failed send signal");
    }

    Ok(poem::Server::new(TcpListener::bind(conf.bind_addr.clone()))
        .run(app)
        .await?)
}
