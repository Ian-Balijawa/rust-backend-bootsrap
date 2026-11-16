//! Redis-backed login throttling. Keys contain only a digest of the account and peer IP.

use redis::{AsyncCommands, aio::ConnectionManager};
use sha2::{Digest, Sha256};

const FAILURE_LIMIT: i64 = 5;
const WINDOW_SECONDS: i64 = 900;

fn failure_key(kind: &str, value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("login:failures:{kind}:{digest:x}")
}

fn account_key(email: &str) -> String {
    failure_key("account", &email.trim().to_lowercase())
}

fn ip_key(ip: &str) -> String {
    failure_key("ip", ip)
}

pub async fn is_login_limited(
    redis: &ConnectionManager,
    email: &str,
    ip: &str,
) -> redis::RedisResult<bool> {
    let mut connection = redis.clone();
    let account_count: Option<i64> = connection.get(account_key(email)).await?;
    let ip_count: Option<i64> = connection.get(ip_key(ip)).await?;
    Ok(account_count.unwrap_or(0) >= FAILURE_LIMIT || ip_count.unwrap_or(0) >= FAILURE_LIMIT)
}

pub async fn record_login_failure(
    redis: &ConnectionManager,
    email: &str,
    ip: &str,
) -> redis::RedisResult<()> {
    let mut connection = redis.clone();
    for key in [account_key(email), ip_key(ip)] {
        let _: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg(0)
            .arg("NX")
            .arg("EX")
            .arg(WINDOW_SECONDS)
            .query_async(&mut connection)
            .await?;
        let _: i64 = connection.incr(&key, 1).await?;
    }
    Ok(())
}

pub async fn clear_login_failures(
    redis: &ConnectionManager,
    email: &str,
    ip: &str,
) -> redis::RedisResult<()> {
    let _: usize = redis.clone().del(&[account_key(email), ip_key(ip)]).await?;
    Ok(())
}

pub async fn request_limited(
    redis: &ConnectionManager,
    action: &str,
    email: &str,
    ip: &str,
    limit: i64,
) -> redis::RedisResult<bool> {
    let mut connection = redis.clone();
    let keys = [
        failure_key(&format!("{action}:email"), &email.trim().to_lowercase()),
        failure_key(&format!("{action}:ip"), ip),
    ];
    let mut limited = false;
    for key in keys {
        let _: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg(0)
            .arg("NX")
            .arg("EX")
            .arg(WINDOW_SECONDS)
            .query_async(&mut connection)
            .await?;
        let count: i64 = connection.incr(&key, 1).await?;
        limited |= count > limit;
    }
    Ok(limited)
}

fn revoked_access_key(token: &str) -> String {
    failure_key("access-revoked", token)
}

pub async fn access_revoked(redis: &ConnectionManager, token: &str) -> redis::RedisResult<bool> {
    let value: Option<u8> = redis.clone().get(revoked_access_key(token)).await?;
    Ok(value.is_some())
}

pub async fn revoke_access(redis: &ConnectionManager, token: &str) -> redis::RedisResult<()> {
    let _: () = redis.clone().set_ex(revoked_access_key(token), 1_u8, 15 * 60).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_key_contains_no_username_or_ip() {
        let key = account_key("Alice");
        assert!(!key.contains("Alice"));
        assert_eq!(key, account_key("alice"));
        assert!(!ip_key("127.0.0.1").contains("127.0.0.1"));
    }
}
