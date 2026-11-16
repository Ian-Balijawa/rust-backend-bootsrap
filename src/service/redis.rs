//! Redis-backed login throttling. Keys contain only a digest of the account and peer IP.

use redis::{AsyncCommands, aio::ConnectionManager};
use sha2::{Digest, Sha256};

const FAILURE_LIMIT: i64 = 5;
const WINDOW_SECONDS: i64 = 900;

fn failure_key(username: &str, ip: &str) -> String {
    let digest = Sha256::digest(format!("{}:{ip}", username.to_lowercase()).as_bytes());
    format!("login:failures:{digest:x}")
}

pub async fn is_login_limited(
    redis: &ConnectionManager,
    username: &str,
    ip: &str,
) -> redis::RedisResult<bool> {
    let count: Option<i64> = redis.clone().get(failure_key(username, ip)).await?;
    Ok(count.unwrap_or(0) >= FAILURE_LIMIT)
}

pub async fn record_login_failure(
    redis: &ConnectionManager,
    username: &str,
    ip: &str,
) -> redis::RedisResult<()> {
    let key = failure_key(username, ip);
    let mut connection = redis.clone();
    let _: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(0)
        .arg("NX")
        .arg("EX")
        .arg(WINDOW_SECONDS)
        .query_async(&mut connection)
        .await?;
    let _: i64 = connection.incr(&key, 1).await?;
    Ok(())
}

pub async fn clear_login_failures(
    redis: &ConnectionManager,
    username: &str,
    ip: &str,
) -> redis::RedisResult<()> {
    let _: usize = redis.clone().del(failure_key(username, ip)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_key_contains_no_username_or_ip() {
        let key = failure_key("Alice", "127.0.0.1");
        assert!(!key.contains("Alice"));
        assert!(!key.contains("127.0.0.1"));
        assert_eq!(key, failure_key("alice", "127.0.0.1"));
    }
}
