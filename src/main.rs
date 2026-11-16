mod api;
mod config;
mod migration;
mod model;
mod scheduler;
mod service;
#[cfg(test)]
mod tests;
mod utils;

use crate::config::Config;
use crate::scheduler::executor::MailTaskExecutor;
use crate::scheduler::task_scheduler::TaskScheduler;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use sea_orm_migration::MigratorTrait;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: redis::aio::ConnectionManager,
    pub config: Config,
    pub scheduler: Arc<TaskScheduler>,
    pub started_at: DateTime<Utc>,
}

#[derive(Parser)]
#[command(name = "axum-template")]
struct Cli {
    #[arg(long, default_value = "0.0.0.0")]
    bind_ip: IpAddr,
    #[arg(long, default_value_t = 8000)]
    port: u16,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Migrate,
    CreateAdmin {
        #[arg(short, long, default_value = "admin")]
        username: String,
        #[arg(short, long)]
        password: Option<String>,
    },
}

async fn init_database(db: &DatabaseConnection) -> anyhow::Result<()> {
    migration::Migrator::up(db, None).await?;

    crate::service::rbac::initialize(db).await?;

    Ok(())
}

async fn create_admin(
    db: &DatabaseConnection,
    username: &str,
    password: Option<String>,
) -> anyhow::Result<()> {
    if user_exists(db, username).await? {
        anyhow::bail!("user already exists: {username}");
    }

    let generated = password.is_none();
    let password = password.unwrap_or_else(generate_password);
    let now = Utc::now();
    let user = crate::model::user::ActiveModel {
        username: Set(username.to_string()),
        password_hash: Set(bcrypt::hash(&password, bcrypt::DEFAULT_COST)?),
        email: Set(None),
        is_active: Set(true),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        last_login_at: Set(None),
        last_login_ip: Set(None),
        ..Default::default()
    }
    .insert(db)
    .await?;
    crate::service::rbac::assign_role(db, user.id, crate::service::rbac::ADMIN_ROLE).await?;
    if generated {
        println!("Generated administrator password: {password}");
    }
    tracing::info!(username, "created administrator account");
    Ok(())
}

fn generate_password() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    let mut value = u128::from_be_bytes(*uuid::Uuid::new_v4().as_bytes());
    let mut password = String::with_capacity(20);
    for _ in 0..20 {
        password.push(ALPHABET[(value % ALPHABET.len() as u128) as usize] as char);
        value /= ALPHABET.len() as u128;
    }
    password
}

async fn shutdown_signal(scheduler: Arc<TaskScheduler>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
    scheduler.stop().await;
}

async fn user_exists(db: &DatabaseConnection, username: &str) -> anyhow::Result<bool> {
    Ok(crate::model::user::Entity::find()
        .filter(crate::model::user::Column::Username.eq(username))
        .one(db)
        .await?
        .is_some())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let bind_address = SocketAddr::new(cli.bind_ip, cli.port);
    // Read Configuration
    let config = Config::from_ini("config.ini")?;

    // Initialize log
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(config.log_level.clone()))
        .compact()
        .init();

    // Initialize the database connection pool
    let db = Database::connect(&config.db_url).await?;

    match cli.command {
        Some(Command::Migrate) => {
            init_database(&db).await?;
            return Ok(());
        }
        Some(Command::CreateAdmin { username, password }) => {
            create_admin(&db, &username, password).await?;
            return Ok(());
        }
        None => {}
    }
    anyhow::ensure!(
        migration::Migrator::get_pending_migrations_read_only(&db)
            .await?
            .is_empty(),
        "database migrations are pending; run `make migrate`"
    );

    let redis_client = redis::Client::open(config.redis_url.as_str())?;
    let mut redis = redis::aio::ConnectionManager::new(redis_client).await?;
    let _: String = redis::cmd("PING").query_async(&mut redis).await?;

    // Initialize the scheduler
    let scheduler = Arc::new(TaskScheduler::with_database(db.clone(), 100));
    // Registering the Mail Executor
    scheduler
        .add_executor(Box::new(MailTaskExecutor {
            config: config.clone(),
        }))
        .await;
    scheduler
        .restore_tasks()
        .await
        .map_err(anyhow::Error::msg)?;
    // Start the scheduler
    scheduler.start().await;

    let state = AppState {
        db,
        redis,
        config,
        scheduler: Arc::clone(&scheduler),
        started_at: Utc::now(),
    };
    let app = api::create_router().with_state(state).layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    );

    tracing::info!(address = %bind_address, "listening");
    let listener = tokio::net::TcpListener::bind(bind_address).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(Arc::clone(&scheduler)))
    .await?;
    Ok(())
}
