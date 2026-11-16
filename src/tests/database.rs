use crate::init_database;
use axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, Database, EntityTrait, Set};
use sha2::{Digest, Sha256};
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
    let configs = crate::repository::system_config::list(&db).await.unwrap();
    assert_eq!(configs.len(), 3);

    let username = format!("test-{}", uuid::Uuid::new_v4());
    let user = crate::model::user::ActiveModel {
        username: Set(username.clone()),
        password_hash: Set(crate::service::user::hash_password("password123")
            .await
            .unwrap()),
        email: Set(Some(format!("{username}@example.com"))),
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
    assert!(!permissions.contains(&"user:manage".to_string()));
    let (token, _) = crate::service::auth::create_session(&db, user.id)
        .await
        .unwrap();
    assert!(
        crate::model::session::Entity::find_by_id(token.clone())
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        crate::repository::session::find_valid(&db, &token)
            .await
            .unwrap()
            .is_some()
    );

    let other = crate::repository::user::insert(
        &db,
        &format!("other-{}", uuid::Uuid::new_v4()),
        crate::service::user::hash_password("password123")
            .await
            .unwrap(),
        None,
    )
    .await
    .unwrap();
    let task = crate::service::task::create(&db, user.id, "private", Utc::now())
        .await
        .unwrap();
    assert!(matches!(
        crate::service::task::get(&db, other.id, task.id).await,
        Err(crate::service::task::TaskError::NotFound)
    ));
    assert!(matches!(
        crate::service::task::update(&db, other.id, task.id, Some("changed".to_string()), None)
            .await,
        Err(crate::service::task::TaskError::NotFound)
    ));
    assert!(matches!(
        crate::service::task::delete(&db, other.id, task.id).await,
        Err(crate::service::task::TaskError::NotFound)
    ));
    let (visible, count) = crate::service::task::list(&db, user.id, 1, 20)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(visible[0].id, task.id);
    crate::service::task::delete(&db, user.id, task.id)
        .await
        .unwrap();
    assert!(matches!(
        crate::service::task::get(&db, user.id, task.id).await,
        Err(crate::service::task::TaskError::NotFound)
    ));
    crate::service::auth::signout(&db, user.id, &token)
        .await
        .unwrap();
    assert!(
        crate::repository::session::find_valid(&db, &token)
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
    crate::service::file::delete(&db, &file.id).await.unwrap();
    assert!(
        crate::service::file::find_for_owner(&db, &dir, user.id, &file.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(path.exists());
    tokio::fs::remove_file(path).await.unwrap();
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

    let http_dir = std::env::temp_dir().join(format!("upload-http-test-{}", uuid::Uuid::new_v4()));
    let scheduler =
        Arc::new(crate::scheduler::task_scheduler::TaskScheduler::with_database(db.clone(), 1));
    let state = crate::AppState {
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
            cors_origins: vec!["http://localhost:3000".to_string()],
            app_env: "development".to_string(),
            jwt_secret: "test-only-secret-longer-than-thirty-two-bytes".to_string(),
            jwt_issuer: "rust-backend-bootstrap".to_string(),
            jwt_audience: "rust-backend-client".to_string(),
        },
        scheduler,
        started_at: Utc::now(),
    };
    let (token, _) = crate::service::auth::issue_access_token(&db, &state.config, user.id)
        .await
        .unwrap();
    let app = crate::routes::file::router().with_state(state.clone());
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
    tokio::fs::remove_file(http_dir.join(id)).await.unwrap();
    tokio::fs::remove_dir(http_dir).await.unwrap();

    let user_app = crate::routes::user::router().with_state(state.clone());
    let peer: std::net::SocketAddr = "127.0.0.1:42000".parse().unwrap();
    let signup_email = format!("signup-{}@example.com", uuid::Uuid::new_v4());
    let mut signup = Request::builder()
        .uri("/signup")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": format!("signup-{}", uuid::Uuid::new_v4()),
                "email": signup_email.to_uppercase(),
                "password": "a-strong-password"
            })
            .to_string(),
        ))
        .unwrap();
    signup.extensions_mut().insert(ConnectInfo(peer));
    let signup_response = user_app.clone().oneshot(signup).await.unwrap();
    assert_eq!(signup_response.status(), StatusCode::CREATED);
    let signup_user = crate::repository::user::find_by_email(&db, &signup_email)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(signup_user.email.as_deref(), Some(signup_email.as_str()));
    let signup_permissions = crate::service::rbac::permissions_for_user(&db, signup_user.id)
        .await
        .unwrap();
    assert!(!signup_permissions.contains(&"user:manage".to_string()));

    for (password, expected) in [
        ("wrong-password", StatusCode::UNAUTHORIZED),
        ("password123", StatusCode::OK),
    ] {
        let mut request = Request::builder()
            .uri("/signin")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"email": format!("{username}@example.com"), "password": password}).to_string(),
            ))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(peer));
        let response = user_app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), expected);
    }

    let (first_refresh, _) = crate::service::auth::create_session(&db, signup_user.id)
        .await
        .unwrap();
    let refreshed = user_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/refresh")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"refresh_token": &first_refresh}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refreshed.status(), StatusCode::OK);
    let refreshed_body = to_bytes(refreshed.into_body(), 4096).await.unwrap();
    let refreshed_json: serde_json::Value = serde_json::from_slice(&refreshed_body).unwrap();
    let second_refresh = refreshed_json["data"]["refresh_token"].as_str().unwrap();
    let access_token = refreshed_json["data"]["token"].as_str().unwrap();
    assert!(
        crate::service::auth::authenticate_access(&db, &state.config, access_token)
            .await
            .unwrap()
            .is_some()
    );
    let reused = user_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/refresh")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"refresh_token": &first_refresh}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reused.status(), StatusCode::UNAUTHORIZED);
    assert!(
        crate::repository::session::find_valid(&db, second_refresh)
            .await
            .unwrap()
            .is_none()
    );

    let (signout_refresh, _) = crate::service::auth::create_session(&db, signup_user.id)
        .await
        .unwrap();
    let (signout_access, _) =
        crate::service::auth::issue_access_token(&db, &state.config, signup_user.id)
            .await
            .unwrap();
    let signed_out = user_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/signout")
                .method("POST")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {signout_access}"))
                .body(Body::from(
                    serde_json::json!({"refresh_token": signout_refresh}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(signed_out.status(), StatusCode::OK);
    let denied = user_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/me")
                .header("authorization", format!("Bearer {signout_access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    let reset_token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let reset_hash = format!("{:x}", Sha256::digest(reset_token.as_bytes()));
    crate::repository::password_reset::insert(
        &db,
        &reset_hash,
        signup_user.id,
        Utc::now() + chrono::Duration::minutes(15),
    )
    .await
    .unwrap();
    let (old_session, _) = crate::service::auth::create_session(&db, signup_user.id)
        .await
        .unwrap();
    crate::service::auth::reset_password(&db, &reset_token, "a-new-strong-password")
        .await
        .unwrap();
    assert!(
        crate::repository::session::find_valid(&db, &old_session)
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        crate::service::auth::reset_password(&db, &reset_token, "another-password").await,
        Err(crate::service::auth::PasswordResetError::InvalidToken)
    ));
    let updated = crate::repository::user::find_by_id(&db, signup_user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        crate::service::user::verify_password(&updated.password_hash, "a-new-strong-password")
            .await
    );
}
