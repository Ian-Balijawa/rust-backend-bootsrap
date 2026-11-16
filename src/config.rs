use configparser::ini::Ini;
use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub db_url: String,
    pub redis_url: String,
    pub upload_dir: PathBuf,
    pub max_upload_bytes: u64,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub log_level: String,
}

impl Config {
    pub fn from_ini(path: &str) -> anyhow::Result<Self> {
        let mut conf = Ini::new();
        conf.load(path).map_err(|e| anyhow::anyhow!(e))?;
        let db_url = env::var("DATABASE_URL")
            .ok()
            .or_else(|| conf.get("database", "url"))
            .ok_or_else(|| anyhow::anyhow!("missing database URL"))?;
        anyhow::ensure!(
            db_url.starts_with("postgres://") || db_url.starts_with("postgresql://"),
            "database URL must use PostgreSQL"
        );
        let redis_url = env::var("REDIS_URL")
            .ok()
            .or_else(|| conf.get("redis", "url"))
            .ok_or_else(|| anyhow::anyhow!("missing Redis URL"))?;
        anyhow::ensure!(
            redis_url.starts_with("redis://") || redis_url.starts_with("rediss://"),
            "invalid Redis URL"
        );
        let upload_dir = env::var_os("UPLOAD_DIR")
            .map(PathBuf::from)
            .or_else(|| conf.get("uploads", "directory").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("uploads"));
        let max_upload_bytes = env::var("MAX_UPLOAD_BYTES")
            .ok()
            .or_else(|| conf.get("uploads", "max_bytes"))
            .unwrap_or_else(|| "10485760".to_string())
            .parse::<u64>()?;
        anyhow::ensure!(
            (1..=1_073_741_824).contains(&max_upload_bytes),
            "MAX_UPLOAD_BYTES must be between 1 and 1073741824"
        );
        let smtp_server = env::var("SMTP_SERVER")
            .ok()
            .or_else(|| conf.get("mail", "smtp_server"))
            .ok_or_else(|| anyhow::anyhow!("missing SMTP_SERVER"))?;
        let smtp_port = env::var("SMTP_PORT")
            .ok()
            .or_else(|| conf.get("mail", "smtp_port"))
            .unwrap_or_else(|| "465".to_string())
            .parse::<u16>()?;
        let smtp_user = env::var("SMTP_USER")
            .ok()
            .or_else(|| conf.get("mail", "smtp_user"))
            .ok_or_else(|| anyhow::anyhow!("missing SMTP_USER"))?;
        let smtp_pass = env::var("SMTP_PASS")
            .ok()
            .or_else(|| conf.get("mail", "smtp_pass"))
            .ok_or_else(|| anyhow::anyhow!("missing SMTP_PASS"))?;
        let log_level = env::var("RUST_LOG")
            .ok()
            .or_else(|| conf.get("log", "level"))
            .unwrap_or_else(|| "info".to_string());
        Ok(Config {
            db_url,
            redis_url,
            upload_dir,
            max_upload_bytes,
            smtp_server,
            smtp_port,
            smtp_user,
            smtp_pass,
            log_level,
        })
    }
}
