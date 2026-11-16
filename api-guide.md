# RUST_API_GUIDE.md

Instructions for bootstrapping and extending a Rust REST API.
Follow every rule here. Do not skip steps.

---

## 1. Stack (fixed choices)

Use these crates. Do not swap them without asking.

- Web framework: axum
- Runtime: tokio
- Database: PostgreSQL with sqlx (compile-checked queries, no ORM)
- Serialization: serde, serde_json
- Validation: validator
- Password hashing: argon2 (Argon2id)
- Tokens: jsonwebtoken
- Errors: thiserror for domain errors, anyhow only in main.rs
- Logging: tracing, tracing-subscriber
- Middleware: tower-http (cors, trace, compression, timeout, request-id)
- Config: dotenvy plus a typed Config struct
- IDs: uuid v7
- Time: chrono with UTC only
- Docs: utoipa plus utoipa-swagger-ui
- Rate limiting: tower-governor
- Tests: tokio::test, reqwest, testcontainers or sqlx::test

---

Layer rules:

- Handler: parse input, validate, call service, return response. No SQL. No business logic.
- Service: business rules, ownership checks, hashing, token logic. No HTTP types.
- Repository: SQL only. No business rules.
- Model: database row structs.
- DTO: request and response structs. Never return a model directly.

---

## 3. Bootstrap steps (do in order)

1. `cargo new api --lib` then add a thin `main.rs`.
2. Add dependencies with `cargo add`. Enable only the features you need.
3. Create `Config` that reads env vars once at startup. Fail fast if one is missing.
4. Create `AppState` holding the PgPool, Config, and shared clients.
5. Create `AppError` with `IntoResponse` (see section 6).
6. Set up tracing with JSON logs in production and pretty logs in dev.
7. Create migrations. Run them at startup in dev only. Run them as a separate step in production.
8. Add `/health` and `/ready` routes.
9. Build the auth feature.
10. Build the demo notes feature. Copy its pattern for all later CRUD.
11. Add OpenAPI docs.
12. Add tests.
13. Add Dockerfile and CI (fmt, clippy, test, sqlx prepare check).

---

## 4. Authentication features

Build all of these:

- POST /api/v1/auth/signup
- POST /api/v1/auth/signin
- POST /api/v1/auth/refresh
- POST /api/v1/auth/signout
- GET /api/v1/auth/me
- POST /api/v1/auth/forgot-password
- POST /api/v1/auth/reset-password
- POST /api/v1/auth/change-password

### Do

- Hash passwords with Argon2id and a per-password random salt.
- Normalize email to lowercase and trim it before save and lookup.
- Add a unique index on the lowercase email.
- Issue a short-lived access token (15 minutes).
- Issue a long-lived refresh token (7 to 30 days).
- Store refresh tokens in the database as a hash, never in plain text.
- Rotate refresh tokens on every refresh. Revoke the old one.
- Detect refresh token reuse and revoke the whole token family.
- Put only user id, role, and expiry in the JWT claims.
- Validate `exp`, `iss`, and `aud` on every token.
- Return the same error for wrong email and wrong password ("Invalid credentials").
- Run a dummy hash check when the user does not exist to keep timing equal.
- Rate limit signin, signup, and forgot-password per IP and per email.
- Lock or slow down an account after repeated failed signins.
- Build an `AuthUser` extractor. Handlers that need auth take it as an argument.
- Support roles (admin, user) from day one.
- Hash reset tokens and make them single use with a short expiry.
- Revoke all refresh tokens after a password change or reset.

### Don't

- Don't store plain text or reversible passwords.
- Don't use MD5, SHA1, SHA256, or bcrypt with a low cost for passwords.
- Don't put secrets, emails, or PII in the JWT.
- Don't hardcode the JWT secret. Load it from env and require 32 or more bytes.
- Don't use `alg: none` or accept a token algorithm chosen by the client.
- Don't reveal whether an email is registered on signin or forgot-password.
- Don't log passwords, tokens, or the Authorization header.
- Don't return the password hash in any response.
- Don't trust a user id from the request body. Take it from the token.

---

## 5. The CRUD pattern (demo feature: notes)

Every resource follows this exact shape. Use `notes` as the template.

Routes:

| Method | Path | Action |
|---|---|---|
| POST | /api/v1/notes | create |
| GET | /api/v1/notes | list with pagination, filter, sort |
| GET | /api/v1/notes/:id | get one |
| PATCH | /api/v1/notes/:id | partial update |
| DELETE | /api/v1/notes/:id | soft delete |

Files per feature:

- `models.rs`: `Note` row struct with `FromRow`.
- `dto.rs`: `CreateNoteRequest`, `UpdateNoteRequest`, `NoteResponse`, `ListNotesQuery`.
- `repository.rs`: `insert`, `find_by_id`, `list`, `update`, `soft_delete`.
- `service.rs`: ownership checks and rules.
- `handlers.rs`: thin HTTP layer.
- `mod.rs`: exports a `router()` function.

Required columns on every table:

- `id` uuid primary key
- `created_at`, `updated_at` timestamptz
- `deleted_at` timestamptz nullable
- `owner_id` when the data belongs to a user

Handler shape:

```rust
pub async fn create_note(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(body): ValidatedJson<CreateNoteRequest>,
) -> Result<(StatusCode, Json<ApiResponse<NoteResponse>>), AppError> {
    let note = notes::service::create(&state, auth.id, body).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::new(note.into()))))
}
```

List response shape:

```json
{
  "data": [],
  "meta": { "page": 1, "per_page": 20, "total": 0, "total_pages": 0 }
}
```

### Do

- Use `ValidatedJson` so validation runs before the handler body.
- Return 201 on create, 200 on read and update, 204 or 200 on delete.
- Scope every query by `owner_id` and `deleted_at IS NULL`.
- Return 404 when a record is missing or belongs to someone else.
- Cap `per_page` (max 100) and default it to 20.
- Whitelist sortable and filterable columns. Map them in code.
- Use `PATCH` with `Option<T>` fields for partial updates.
- Use `COALESCE($1, column)` or a query builder for patch updates.
- Set `updated_at` in the update query.
- Wrap multi-step writes in a transaction.
- Use one `ApiResponse<T>` envelope everywhere.

### Don't

- Don't copy-paste a new pattern per resource. Reuse this one.
- Don't expose database models directly.
- Don't use `SELECT *`. List columns.
- Don't build SQL with `format!` or string concatenation.
- Don't use `OFFSET` on very large tables. Use keyset pagination there.
- Don't hard delete user data unless the spec says so.
- Don't skip the ownership check because "the route is protected".
- Don't return 403 for records the user should not know exist.

---

## 6. Error handling

Define one error enum.

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("validation failed")]
    Validation(validator::ValidationErrors),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}
```

Error response shape:

```json
{
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "Invalid input",
    "details": [{ "field": "email", "message": "must be a valid email" }]
  },
  "request_id": "..."
}
```

### Do

- Implement `IntoResponse` once and map each variant to a status code.
- Map unique-violation database errors to 409.
- Log the full error server-side with the request id.
- Return a generic message for 500 errors.
- Use `?` everywhere. Convert errors with `From`.

### Don't

- Don't use `unwrap()` or `expect()` in request paths.
- Don't use `panic!` for business errors.
- Don't leak SQL errors, stack traces, or file paths to the client.
- Don't return 200 with an error body.
- Don't use `Box<dyn Error>` in handlers or services.

---

## 7. Database rules

### Do

- Write every schema change as a new migration. Never edit an applied one.
- Use `sqlx::query_as!` with `cargo sqlx prepare` so CI builds offline.
- Add indexes for foreign keys, `owner_id`, and filter columns.
- Add partial indexes for `deleted_at IS NULL` when it helps.
- Use a connection pool with explicit max connections and acquire timeout.
- Use `timestamptz`, `uuid`, and `text` with `CHECK` constraints.
- Use `ON DELETE` rules on every foreign key.

### Don't

- Don't hold a transaction open across an HTTP call or `.await` on slow work.
- Don't run queries inside loops. Batch or join.
- Don't store money as float. Use integer minor units or `numeric`.
- Don't store local time. Store UTC.
- Don't run migrations automatically on every production instance at once.

---

## 8. Async and performance

### Do

- Use `tokio::task::spawn_blocking` for CPU heavy work.
- Set request timeouts with `tower_http::timeout`.
- Set a body size limit.
- Enable gzip or brotli compression.
- Share clients (HTTP, Redis) through `AppState`. Build them once.
- Handle graceful shutdown on SIGTERM and SIGINT.

### Don't

- Don't block the async runtime with `std::thread::sleep`, sync file IO, or heavy loops.
- Don't use `std::sync::Mutex` across an `.await`.
- Don't clone large data. Use `Arc`.
- Don't spawn tasks without a plan for errors and shutdown.

---

## 9. Security

### Do

- Configure CORS with an explicit origin list from env.
- Add security headers (HSTS, X-Content-Type-Options, X-Frame-Options).
- Validate and bound every input (length, range, format).
- Run `cargo audit` and `cargo deny` in CI.
- Keep secrets in env or a secret manager.
- Provide `.env.example` with no real values.
- Run the container as a non-root user.
- Use TLS at the edge and between services where possible.

### Don't

- Don't use `CorsLayer::permissive()` in production.
- Don't commit `.env` files.
- Don't log request bodies for auth routes.
- Don't trust `X-Forwarded-For` unless a known proxy sets it.
- Don't use `unsafe`. If you must, document why.

---

## 10. API conventions

- Prefix all routes with `/api/v1`.
- Use plural nouns for resources.
- Use snake_case in JSON fields.
- Use RFC 3339 for timestamps.
- Use cursor or page based pagination. Pick one and keep it.
- Return a `request_id` header on every response.
- Document every route with `utoipa` and keep Swagger in sync.
- Version breaking changes under `/api/v2`.

---

## 11. Testing

### Do

- Write unit tests for services and pure functions.
- Write integration tests that hit the real router with a real test database.
- Test auth flows end to end: signup, signin, refresh, reuse detection, signout.
- Test that user A cannot read, update, or delete user B's notes.
- Test validation failures and error shapes.
- Run `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` in CI.

### Don't

- Don't mock the database for repository tests.
- Don't share state between tests. Use a fresh database or transaction per test.
- Don't ship a feature without an auth and ownership test.

---

## 12. Code quality

- Keep functions short and single purpose.
- Prefer `&str` and borrowing in function signatures.
- Derive `Debug`, `Clone`, `Serialize`, `Deserialize` only where needed.
- Use newtypes for IDs and emails when it prevents mix-ups.
- Name things for what they do. Avoid abbreviations.
- Comment the why, not the what.
- Keep `main.rs` under 50 lines.
- Fix every clippy warning. Do not silence with `#[allow]` without a comment.

---

## 13. Adding a new resource (checklist)

1. Create migration with required columns and indexes.
2. Copy the `notes` feature folder and rename.
3. Update models, DTOs, and validation rules.
4. Update repository queries. Keep the owner and soft delete filters.
5. Update service rules.
6. Register the router in `routes.rs`.
7. Add OpenAPI annotations.
8. Add tests including the ownership test.
9. Run fmt, clippy, test, and `cargo sqlx prepare`.

---

## 14. Final rules for the assistant

- Ask before adding a dependency not listed in section 1.
- Never change the response envelope or error shape without approval.
- Never skip tests to finish faster.
- Never leave `TODO` in security code.
- When unsure, pick the simpler option and say what you chose.
- Output complete files for new modules. Output small diffs for targeted fixes.
