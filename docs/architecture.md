# Backend module layout

This service exposes JSON APIs for a separate React client. It has no server-rendered views or template layer.

```text
src/
├── routes/       Axum paths and HTTP method registration
├── controller/   Request extraction, validation, authorization, response mapping
├── service/      Authentication, RBAC, file storage, Redis throttling, user workflows
├── repository/   PostgreSQL access for users, sessions, tasks, files, config, logs, scheduler
├── model/        SeaORM entities and permission declarations
├── http/         Shared Bearer extractors and JSON response/error types
├── migration/    Versioned PostgreSQL schema changes
├── scheduler/    Task scheduling and execution logic
├── utils/        External utilities such as SMTP
├── config.rs     Environment and INI configuration
└── main.rs       Startup, dependency wiring, and shutdown
```

A typical request flows from a `routes` declaration to a `controller`. The controller parses HTTP input and enforces route access, calls a `service` for a workflow or a `repository` for simple CRUD, then returns a JSON response. Services use repositories for application table persistence. The scheduler uses `repository::scheduler` for its task and execution records. SeaORM models describe the database shape; they are not UI models or views.

For example, `POST /user/login` is declared in `routes/user.rs`, translated from HTTP in `controller/user.rs`, coordinated in `service/auth.rs`, and persisted through `repository/user.rs`, `repository/session.rs`, and `repository/login_log.rs`. Redis throttling lives in `service/redis.rs`.

To add a feature, add its SeaORM entity and migration if it needs storage, a repository for data access, a service for workflows, a controller for HTTP input/output, and a route declaration. For a simple read or CRUD operation, the controller may call the repository directly. Shared authentication and response types live in `http/`.

The React client should consume the documented JSON API. It is deployed independently of this backend; this repository contains no JSX, HTML templates, or static UI routes.
