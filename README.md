# Axum backend bootstrap

A Rust JSON API starter for a separate React client, with Axum, SeaORM, PostgreSQL, Redis, role-based access control, persistent scheduling, SMTP mail, and local file storage. See the [module layout](docs/architecture.md).

## Quick start

Requires Rust and Docker Compose. `make up` starts PostgreSQL and Redis on loopback ports 15432 and 16379, leaving the standard ports free for other local services. Override the published ports with `POSTGRES_HOST_PORT` and `REDIS_HOST_PORT`, and set matching `DATABASE_URL` and `REDIS_URL` values for the API.

```sh
make services
cp .env.example .env
make dev
```

`make dev` starts the services if needed, applies migrations and RBAC seeds, then starts the API on `0.0.0.0:8000`. `make migrate` runs migrations without starting the API. The API refuses to start while migrations are pending. `make verify` runs formatting, Clippy, and unit tests. `make down` stops the containers without deleting their volumes. If you already have a `.env`, check its `DATABASE_URL` and `REDIS_URL`: they override the values in `config.ini` and must match the Compose ports and credentials.

The API hardening migration invalidates older plaintext sessions, so users must sign in again after it is applied. It also adds a case-insensitive unique email index; resolve any existing emails that differ only in letter case before migrating.

The sample `config.ini` contains local development defaults. The `.env.example` file includes a local placeholder for `JWT_SECRET`; replace it with a random secret of at least 32 bytes before deployment. Set explicit `CORS_ORIGINS` values. `MAX_UPLOAD_BYTES` defaults to 10 MiB and accepts 1 byte through 1 GiB. Configure a persistent, private upload directory. Do not expose PostgreSQL or Redis publicly.

Create an admin account explicitly:

```sh
cargo run -- create-admin --username admin
```

If `--password` is omitted, the generated password is printed once. Public signups always receive the user role; use the CLI to create administrators.

## API

Browse the API reference at `http://localhost:8000/swagger-ui/` or download its OpenAPI JSON from `http://localhost:8000/api-docs/openapi.json`.

- `GET /live`: process liveness.
- `GET /health`: checks PostgreSQL and Redis and reports scheduler state.
- `GET /ready`: readiness check for PostgreSQL and Redis.
- `POST /api/v1/auth/signup`, `POST /api/v1/auth/signin`, `POST /api/v1/auth/refresh`, `POST /api/v1/auth/signout`, `GET /api/v1/auth/me`, `POST /api/v1/auth/change-password`, `POST /api/v1/auth/forgot-password`, `POST /api/v1/auth/reset-password`: user access. Signin accepts an email and password and returns a 15-minute JWT access token plus a 14-day rotating refresh token. Passwords use Argon2id; existing bcrypt hashes are upgraded at signin. Reset emails use single-use 15-minute tokens.
- `GET/POST /api/v1/tasks`, `GET/PATCH/DELETE /api/v1/tasks/{id}`: owner-scoped task data. Lists accept `page` and `per_page` (up to 100). Deletion is soft.
- `GET/POST /api/v1/notes`, `GET/PATCH/DELETE /api/v1/notes/{id}`: owner-scoped notes with UUIDv7 IDs, pagination, title search, sort, and soft deletion. This resource uses compile-checked SQLx queries.
- `/api/v1/scheduler`, `/api/v1/system-config`: scheduler and configuration management.
- `POST /api/v1/files/`, `GET /api/v1/files/{id}`, `GET /api/v1/files/{id}/download`, `DELETE /api/v1/files/{id}`: file upload, metadata, download, and soft deletion.

Protected routes require `Authorization: Bearer <token>`. File routes require `file:read` or `file:write`; only owners can access a file. Upload a raw request body with an `X-File-Name` header and a `Content-Type` header:

```sh
curl -X POST http://localhost:8000/api/v1/files/ \
  -H "Authorization: Bearer $TOKEN" \
  -H 'X-File-Name: report.pdf' \
  -H 'Content-Type: application/pdf' \
  --data-binary @report.pdf
```

The response contains a UUIDv7 file ID. Download with `curl -OJ -H "Authorization: Bearer $TOKEN" http://localhost:8000/api/v1/files/ID/download`. Uploads stream to a temporary file and enforce the configured size limit while streaming. Soft deletion hides file metadata but retains the bytes under `UPLOAD_DIR` for later cleanup or recovery. Back up both the database and upload directory together. See [API guide](docs/api.md) and [scheduler guide](docs/scheduler_usage.md).

## Checks and deployment

`make test-integration` requires `TEST_DATABASE_URL` pointing to a disposable PostgreSQL database and `TEST_REDIS_URL` pointing to Redis. It exercises migrations, RBAC, sessions, upload storage, and login throttling. Unit tests use SQLite only for isolated scheduler tests; the application configuration accepts PostgreSQL URLs only.

Run `make migrate` before starting the API. A deployment can use separate migration and runtime database roles so the API does not need schema privileges. Restrict the upload directory to the API process identity. Terminate TLS at a reverse proxy, set request limits there, and arrange encrypted offsite backups, restore drills, monitoring, and alerts. See [audit](docs/best-practices-audit.md) for current repository coverage and outstanding operations work.
