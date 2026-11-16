# Axum backend bootstrap

A Rust JSON API starter for a separate React client, with Axum, SeaORM, PostgreSQL, Redis, role-based access control, persistent scheduling, SMTP mail, and local file storage. See the [module layout](docs/architecture.md).

## Quick start

Requires Rust and Docker Compose. `make services` starts PostgreSQL and Redis on loopback ports 15432 and 16379, leaving the standard ports free for other local services. Override the published ports with `POSTGRES_HOST_PORT` and `REDIS_HOST_PORT`, and set matching `DATABASE_URL` and `REDIS_URL` values for the API.

```sh
make services
make dev
```

`make dev` starts the services if needed, applies migrations and RBAC seeds, then starts the API on `0.0.0.0:8000`. `make migrate` runs migrations without starting the API. The API refuses to start while migrations are pending. `make verify` runs formatting, Clippy, and unit tests. `make services-down` stops the containers without deleting their volumes.

The sample `config.ini` contains local development defaults. In deployment set `DATABASE_URL`, `REDIS_URL`, `UPLOAD_DIR`, `MAX_UPLOAD_BYTES`, `SMTP_SERVER`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASS`, and optionally `RUST_LOG` through the environment. `MAX_UPLOAD_BYTES` defaults to 10 MiB and accepts 1 byte through 1 GiB. Configure a persistent, private upload directory. Do not expose PostgreSQL or Redis publicly.

Create an admin account explicitly:

```sh
cargo run -- create-admin --username admin
```

If `--password` is omitted, the generated password is printed once. The first registered user also receives the admin role; subsequent users receive the user role.

## API

- `GET /live`: process liveness.
- `GET /health`: checks PostgreSQL and Redis and reports scheduler state.
- `POST /user/register`, `POST /user/login`, `POST /user/logout`, `GET /user/me`: user access. Login accepts a username and password, returns a 24-hour bearer token, and limits failed attempts per username and peer IP with Redis.
- `GET/POST /tasks`, `GET/PUT/DELETE /tasks/{id}`: task data.
- `GET/POST /scheduler/tasks`, `GET/DELETE /scheduler/tasks/{id}`, `POST /scheduler/tasks/{id}/run`: scheduler management.
- `GET/POST/PUT/DELETE /system-config/{key}`: system configuration.
- `POST /files/`, `GET /files/{id}`, `GET /files/{id}/download`, `DELETE /files/{id}`: file upload, metadata, download, and deletion.

Protected routes require `Authorization: Bearer <token>`. File routes require `file:read` or `file:write`; owners and administrators can access a file. Upload a raw request body with an `X-File-Name` header and a `Content-Type` header:

```sh
curl -X POST http://localhost:8000/files/ \
  -H "Authorization: Bearer $TOKEN" \
  -H 'X-File-Name: report.pdf' \
  -H 'Content-Type: application/pdf' \
  --data-binary @report.pdf
```

The response contains a UUID file ID. Download with `curl -OJ -H "Authorization: Bearer $TOKEN" http://localhost:8000/files/ID/download`. Uploads stream to a temporary file and enforce the configured size limit while streaming. The PostgreSQL `files` table stores metadata; file bytes live under `UPLOAD_DIR` with UUID names. Back up both the database and upload directory together. See [API guide](docs/api.md) and [scheduler guide](docs/scheduler_usage.md).

## Checks and deployment

`make test-integration` requires `TEST_DATABASE_URL` pointing to a disposable PostgreSQL database and `TEST_REDIS_URL` pointing to Redis. It exercises migrations, RBAC, sessions, upload storage, and login throttling. Unit tests use SQLite only for isolated scheduler tests; the application configuration accepts PostgreSQL URLs only.

Run `make migrate` before starting the API. A deployment can use separate migration and runtime database roles so the API does not need schema privileges. Restrict the upload directory to the API process identity. Terminate TLS at a reverse proxy, set request limits there, and arrange encrypted offsite backups, restore drills, monitoring, and alerts. See [audit](docs/best-practices-audit.md) for current repository coverage and outstanding operations work.
