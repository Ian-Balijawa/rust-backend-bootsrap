mod bootstrap;
mod config;
mod controller;
mod http;
mod migration;
mod model;
mod openapi;
mod repository;
mod routes;
mod scheduler;
mod service;
#[cfg(test)]
mod tests;
mod utils;

pub use bootstrap::AppState;
#[cfg(test)]
pub(crate) use bootstrap::{generate_password, init_database};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bootstrap::run().await
}
