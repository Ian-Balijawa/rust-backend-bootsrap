use crate::init_database;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, Database, Set};
use std::sync::Arc;
use tower::ServiceExt;

#[test]
fn generated_admin_password_is_alphanumeric() {
    let password = crate::generate_password();
    assert_eq!(password.len(), 20);
    assert!(password.bytes().all(|byte| byte.is_ascii_alphanumeric()));
}

/// Requires an empty, disposable PostgreSQL database and Redis instance.
#[tokio::test]
#[ignore = "set TEST_DATABASE_URL and TEST_REDIS_URL to disposable services"]
async fn postgres_redis_and_file_roundtrip() {
    let database_url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
    let redis_url = std::env::var("TEST_REDIS_URL").expect("TEST_REDIS_URL");
    let db = Database::connect(database_url).await.unwrap();
    init_database(&db).await.unwrap();
    init_database(&db).await.unwrap();
    let configs = crate::service::system_config::list(&db).await.unwrap();
    assert_eq!(configs.len(), 3);

    let username = format!("test-{}", uuid::Uuid::new_v4());
    let user = crate::model::user::ActiveModel {
        username: Set(username.clone()),
        password_hash: Set("hash".to_string()),
        email: Set(None),
        is_active: Set(true),
        created_at: Set(Some(Utc::now())),
        updated_at: Set(Some(Utc::now())),
        last_login_at: Set(None),
        last_login_ip: Set(None),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    crate::service::rbac::assign_registration_role(&db, user.id)
        .await
        .unwrap();
    let permissions = crate::service::rbac::permissions_for_user(&db, user.id)
        .await
        .unwrap();
    assert!(permissions.contains(&"file:read".to_string()));
    assert!(permissions.contains(&"file:write".to_string()));
    let (token, _) = crate::service::auth::create_session(&db, user.id)
        .await
        .unwrap();
    assert!(
        crate::service::auth::authenticate(&db, &token)
            .await
            .unwrap()
            .is_some()
    );
    crate::service::auth::delete_session(&db, &token)
        .await
        .unwrap();
    assert!(
        crate::service::auth::authenticate(&db, &token)
            .await
            .unwrap()
            .is_none()
    );

    let dir = std::env::temp_dir().join(format!("upload-test-{}", uuid::Uuid::new_v4()));
    let oversized = crate::service::file::save(
        &db,
        &dir,
        user.id,
        "hello.txt",
        "text/plain",
        3,
        Body::from("hello"),
    )
    .await;
    assert!(matches!(
        oversized,
        Err(crate::service::file::UploadError::TooLarge)
    ));
    let file = crate::service::file::save(
        &db,
        &dir,
        user.id,
        "hello.txt",
        "text/plain",
        100,
        Body::from("hello"),
    )
    .await
    .unwrap();
    assert_eq!(file.size_bytes, 5);
    let path = crate::service::file::path_for(&dir, &file.id).unwrap();
    assert_eq!(tokio::fs::read(&path).await.unwrap(), b"hello");
    crate::service::file::delete(&db, &dir, &file.id)
        .await
        .unwrap();
    assert!(!path.exists());
    tokio::fs::remove_dir(dir).await.unwrap();

    let client = redis::Client::open(redis_url).unwrap();
    let redis = redis::aio::ConnectionManager::new(client).await.unwrap();
    let ip = uuid::Uuid::new_v4().to_string();
    assert!(
        !crate::service::redis::is_login_limited(&redis, &username, &ip)
            .await
            .unwrap()
    );
    for _ in 0..5 {
        crate::service::redis::record_login_failure(&redis, &username, &ip)
            .await
            .unwrap();
    }
    assert!(
        crate::service::redis::is_login_limited(&redis, &username, &ip)
            .await
            .unwrap()
    );
    crate::service::redis::clear_login_failures(&redis, &username, &ip)
        .await
        .unwrap();
    assert!(
        !crate::service::redis::is_login_limited(&redis, &username, &ip)
            .await
            .unwrap()
    );

    let (token, _) = crate::service::auth::create_session(&db, user.id)
        .await
        .unwrap();
    let http_dir = std::env::temp_dir().join(format!("upload-http-test-{}", uuid::Uuid::new_v4()));
    let scheduler =
        Arc::new(crate::scheduler::task_scheduler::TaskScheduler::with_database(db.clone(), 1));
    let app = crate::api::file::router().with_state(crate::AppState {
        db: db.clone(),
        redis: redis.clone(),
        config: crate::config::Config {
            db_url: "postgres://test".to_string(),
            redis_url: "redis://test".to_string(),
            upload_dir: http_dir.clone(),
            max_upload_bytes: 5,
            smtp_server: String::new(),
            smtp_port: 465,
            smtp_user: String::new(),
            smtp_pass: String::new(),
            log_level: "info".to_string(),
        },
        scheduler,
        started_at: Utc::now(),
    });
    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .method("POST")
                .body(Body::from("hi"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let too_large = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .method("POST")
                .header("authorization", format!("Bearer {token}"))
                .header("x-file-name", "hello.txt")
                .body(Body::from("123456"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let uploaded = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .method("POST")
                .header("authorization", format!("Bearer {token}"))
                .header("x-file-name", "hello.txt")
                .header("content-type", "text/plain")
                .body(Body::from("hello"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(uploaded.status(), StatusCode::CREATED);
    let body = to_bytes(uploaded.into_body(), 4096).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = payload["data"]["id"].as_str().unwrap();
    let downloaded = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/{id}/download"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(downloaded.status(), StatusCode::OK);
    assert!(
        downloaded.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("filename=\"hello.txt\"")
    );
    assert_eq!(
        to_bytes(downloaded.into_body(), 4096).await.unwrap(),
        "hello"
    );
    let deleted = app
        .oneshot(
            Request::builder()
                .uri(format!("/{id}"))
                .method("DELETE")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::OK);
    tokio::fs::remove_dir(http_dir).await.unwrap();
}
