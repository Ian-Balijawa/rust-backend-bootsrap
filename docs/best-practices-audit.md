# Best-practices audit

This audit reflects this repository as of 2026-11-16. The previous version described a separate MySQL application and is not evidence for this project.

| Area | Current state | Follow-up |
| --- | --- | --- |
| Architecture | Axum handlers, SeaORM models and migrations, services, and scheduler are separated. Files have their own API, model, service, and migration. | Keep feature boundaries clear as the scaffold grows. |
| PostgreSQL | The application accepts PostgreSQL URLs, uses SeaORM for sessions, and has versioned migrations. `make migrate` applies migrations and RBAC seeds. Runtime checks for pending migrations without schema changes. | Use separate migration and runtime database roles in deployment. |
| Redis | Startup and `/health` verify Redis. Login failures are limited to five per username and TCP peer IP for 15 minutes; keys contain a SHA-256 digest rather than the username or IP. | Monitor availability; login fails closed if Redis is unavailable. Add broader per-IP request limits at the proxy. |
| Authentication and authorization | Passwords use bcrypt, sessions are stored in PostgreSQL, and routes check RBAC permissions. File access checks ownership or admin permission. | Add a password policy, registration controls, account-level lockout, and session cleanup if required by the product. |
| Upload security | Raw-body uploads stream to private local storage with UUID paths. Filename validation, streaming size enforcement, owner checks, and temporary-file cleanup are implemented. | Add malware scanning and storage quotas for public or untrusted deployments. Keep upload storage out of the web root. |
| Configuration | PostgreSQL, Redis, upload path, and upload size can be set with environment variables. Sample values are for local development. SMTP credentials are not logged. | Put production secrets in a secret manager or protected environment files. |
| Testing | Unit tests cover scheduler behavior, path traversal rejection, and key privacy. CI runs an ignored PostgreSQL/Redis test against disposable services for migrations, RBAC, sessions, file storage, and throttling. | Add HTTP behavior and load tests as requirements emerge. |
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
