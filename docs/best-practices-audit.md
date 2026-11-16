# Best-practices audit

This audit reflects this repository as of 2026-11-16. The previous version described a separate MySQL application and is not evidence for this project.

| Area | Current state | Follow-up |
| --- | --- | --- |
| Architecture | Routes, controllers, HTTP support, repositories, services, SeaORM models, migrations, and scheduler are separated. The React client is external; there are no server-rendered views. | Keep feature boundaries clear as the scaffold grows. |
| PostgreSQL | The application accepts PostgreSQL URLs, uses SeaORM with versioned migrations, and checks for pending migrations at runtime. The new migration adds case-insensitive email uniqueness, soft deletion columns, and reset token storage. | Use separate migration and runtime database roles in deployment. SQLx-only persistence from the guide is still outstanding. |
| Redis | Startup and `/health` verify Redis. Failed signins are limited independently by email and TCP peer IP; signup and password reset requests are also limited. Keys contain digests rather than raw identifiers. | Monitor availability and add broader edge rate limits. |
| Authentication and authorization | New passwords use Argon2id. Existing bcrypt hashes are upgraded on signin. Access uses 15-minute JWTs; 14-day refresh tokens are hashed, rotated, and grouped into families for reuse detection. Signout revokes the access token in Redis. Reset tokens are hashed, single-use, and expire after 15 minutes. Public signup always gets the user role. Tasks and files are owner-scoped. | Add a policy for cleanup of expired refresh and reset records. |
| Upload security | Raw-body uploads stream to private local storage with UUIDv7 paths. Filename validation, streaming size enforcement, owner checks, and temporary-file cleanup are implemented. File deletion is soft and retains bytes. | Add a retention and cleanup process for soft-deleted bytes. |
| Configuration | PostgreSQL, Redis, SMTP, CORS origins, upload path, and size can be set through environment variables. Development logs are pretty; production logs are JSON. | Put production secrets in a secret manager or protected environment files. |
| Testing | Unit tests cover scheduler behavior, Argon2id, path traversal, and key privacy. The PostgreSQL/Redis integration test covers migrations, signup, role assignment, sessions, password reset, file storage, owner isolation, and throttling. | Add request-level middleware tests and full refresh-flow tests when JWT refresh tokens are introduced. |
| Logging | HTTP tracing and structured application events are present. Login keys exclude raw identifiers. | Send logs to a central store and set retention and alert policies. Review other event fields before production use. |
| Health | `/live` checks the process; `/health` checks PostgreSQL, Redis, and reports scheduler state. | Add external uptime checks and host/application metrics. |
| Format and lint | `make verify` and CI run rustfmt, Clippy with warnings denied, and unit tests. CI runs `cargo audit` with one documented exception: RUSTSEC-2026-0459 affects the SQLite library used only by in-memory scheduler tests; SQLx 0.9 pins its SQLite binding below the fixed version. The production database is PostgreSQL. | Remove the exception when the upstream SQLx/SeaORM stack permits a fixed SQLite binding. Review new advisories and keep dependencies current. |
| Transport and deployment | Compose binds local service ports to loopback. API bind address is configurable. | Terminate HTTPS at a proxy, set body and request rate limits, restrict firewall and database access. |
| Backups | File metadata is in PostgreSQL; bytes are in `UPLOAD_DIR`. | Back up both together, encrypt offsite, retain versions, and test restores. |

## Deployment checklist

1. Set production `DATABASE_URL`, `REDIS_URL`, `UPLOAD_DIR`, and SMTP values. Keep secrets outside the repository.
2. Run `make migrate` with a migration role, then run the API with a runtime role that has only the required DML grants.
3. Give the API process private read/write access to `UPLOAD_DIR`; persist and back it up with PostgreSQL.
4. Put the API behind HTTPS and a reverse proxy with request size and rate limits. Keep PostgreSQL and Redis private.
5. Configure offsite backups, restore drills, uptime monitoring, metrics, alerts, and log retention.
