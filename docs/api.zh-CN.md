# API 文档

服务默认监听 `0.0.0.0:8000`，本机可通过 `http://127.0.0.1:8000` 访问。除注册、登录和健康检查外，接口均使用：

```text
Authorization: Bearer <token>
Content-Type: application/json
```

统一响应结构：

```json
{"success":true,"message":"ok","data":{}}
```

错误使用对应 HTTP 状态码：`400` 参数错误、`401` 未登录或会话失效、`403` 权限不足、`404` 资源不存在、`500` 服务错误。

## 用户与认证

### `POST /api/v1/auth/signup`（公开）

```json
{"username":"admin","password":"password123","email":"admin@example.com"}
```

密码至少 8 位。公开注册的用户始终获得 `user` 角色；管理员通过 CLI 创建。邮箱会去除首尾空格并转为小写。

### `POST /api/v1/auth/signin`（公开）

```json
{"email":"admin@example.com","password":"password123"}
```

返回 `token`、`refresh_token`、到期时间及用户信息。JWT 访问 token 有效期 15 分钟，刷新 token 有效期 14 天。登录 IP 来自 TCP 对端地址，不信任客户端代理头。

`POST /api/v1/auth/refresh` 使用 `refresh_token` 换取新的 token 对；刷新 token 每次轮换，重复使用旧 token 会撤销整个 token 家族。

### `GET /api/v1/auth/me`（登录）

返回当前用户的 `id`、`username`、`email`、`is_active`、审计时间、`last_login_ip` 和权限列表。

### `POST /api/v1/auth/signout`（登录）

请求体提供 `refresh_token`。当前访问 token 立即失效，刷新 token 被撤销。

`POST /api/v1/auth/change-password` 使用 `current_password` 和 `new_password` 修改密码，并撤销所有会话。`POST /api/v1/auth/forgot-password` 使用 `email` 请求重置邮件；`POST /api/v1/auth/reset-password` 使用 `token` 和 `new_password` 完成重置。重置 token 15 分钟内有效且只能使用一次。

### `PUT /api/v1/auth/{id}/role`（`user:manage`）

```json
{"role":"admin"}
```

替换用户的单个主角色。SeaORM RBAC 每位用户只绑定一个主角色；系统拒绝移除最后一个管理员。

## 普通任务

每位用户只能操作自己的任务；访问其他用户的任务返回 404。

- `GET /api/v1/tasks`：`task:read`，支持 `page` 和 `per_page`（最多 100）
- `GET /api/v1/tasks/{id}`：`task:read`
- `POST /api/v1/tasks`：`task:write`
- `PATCH /api/v1/tasks/{id}`：`task:write`
- `DELETE /api/v1/tasks/{id}`：`task:write`

创建：

```json
{"action":"send_report","schedule_time":"2026-07-12T10:00:00Z"}
```

创建时用户 ID 来自 token。更新时 `action`、`schedule_time` 均为可选字段。删除为软删除。

## 调度器

- `GET /api/v1/scheduler/tasks`：`scheduler:read`
- `GET /api/v1/scheduler/tasks/{id}`：`scheduler:read`
- `GET /api/v1/scheduler/tasks/{id}/executions`：`scheduler:read`
- `POST /api/v1/scheduler/tasks`：`scheduler:write`
- `POST /api/v1/scheduler/tasks/{id}/run`：`scheduler:write`
- `DELETE /api/v1/scheduler/tasks/{id}`：`scheduler:write`

创建循环任务：

```json
{
  "name":"cleanup",
  "executor_type":"cleanup_executor",
  "task_type":"recurring",
  "interval_seconds":60,
  "timeout_seconds":30,
  "max_retries":3,
  "data":{"scope":"expired"}
}
```

`task_type` 可为 `one_time`、`recurring`、`scheduled`、`persistent`。Scheduled 使用 `next_run`，Persistent executor 应持续运行直到任务被删除或服务停止。

创建时会校验 `executor_type` 是否已经注册，未注册返回 HTTP `400`。任务定义写入 `scheduler_tasks`，每次执行写入 `task_executions`，其中保存 `running`、`completed`、`failed`、`cancelled` 状态、结束时间和错误信息。

`GET /api/v1/scheduler/tasks/{id}/executions` 按时间倒序返回执行记录：

```json
{"success":true,"message":"ok","data":[{"id":"...","task_id":"...","attempt":1,"status":"completed","started_at":"2026-09-06T08:00:00Z","finished_at":"2026-09-06T08:00:02Z","error":null}]}
```

## 系统配置

- `GET /api/v1/system-config`：`system_config:read`
- `GET /api/v1/system-config/{key}`：`system_config:read`
- `POST /api/v1/system-config/{key}`：`system_config:write`，创建
- `PUT /api/v1/system-config/{key}`：`system_config:write`，更新
- `DELETE /api/v1/system-config/{key}`：`system_config:write`

创建或更新请求：

```json
{"content":"配置内容，可以是 JSON 字符串或普通文本"}
```

## 系统状态

- `GET /health`：公开
- `GET /app-info`：`system_config:read`

## 权限扩展

权限码和类型标记位于 `src/http/auth.rs`。权限数据由 SeaORM RBAC 的 `sea_orm_*` 表维护，逻辑资源为 `api`；新增受保护接口时定义标记，并把 `Required<权限类型>` 放入 handler 参数即可同时完成认证和授权。

## 文件服务

文件接口需要 Bearer token。用户仅能操作自己的文件；访问其他用户的文件返回 404。

- `POST /api/v1/files/`（`file:write`）：使用原始请求体上传，提供 `X-File-Name`（最多 255 字节，不能包含路径分隔符），可提供 `Content-Type`。成功返回 201 和文件元数据，超过 `MAX_UPLOAD_BYTES` 返回 413。
- `GET /api/v1/files/{id}`（`file:read`）：获取元数据。
- `GET /api/v1/files/{id}/download`（`file:read`）：流式下载文件。
- `DELETE /api/v1/files/{id}`（`file:write`）：软删除元数据；文件内容保留在私有存储中。

`GET /live` 检查进程；`GET /health` 检查 PostgreSQL 和 Redis，依赖不可用时返回 503。相同用户名和 TCP 对端 IP 在 15 分钟内登录失败五次后，登录接口返回 429。
