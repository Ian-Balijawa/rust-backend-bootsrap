# API reference

The API listens on port 8000 by default. Resource routes use `/api/v1`; health routes remain at the root. Authenticated requests send `Authorization: Bearer <token>`. Responses retain the existing `{"success":true,"message":"ok","data":...}` envelope. Errors use a non-2xx status with `success:false`.

Every response from the server includes an `x-request-id` header. Set `CORS_ORIGINS` to a comma-separated list of allowed client origins.

## Authentication

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/v1/auth/signup` | Create a user account with `username`, `email`, and `password` |
| POST | `/api/v1/auth/signin` | Sign in with `email` and `password` |
| POST | `/api/v1/auth/refresh` | Rotate a `refresh_token` and receive a new token pair |
| POST | `/api/v1/auth/signout` | Revoke the current bearer token and the supplied `refresh_token` |
| GET | `/api/v1/auth/me` | Read the current profile |
| POST | `/api/v1/auth/change-password` | Change password with `current_password` and `new_password` |
| POST | `/api/v1/auth/forgot-password` | Request a reset email with `email` |
| POST | `/api/v1/auth/reset-password` | Reset with `token` and `new_password` |
| PUT | `/api/v1/auth/{id}/role` | Admin: assign `admin` or `user` |

Passwords require 8 to 1024 characters. Email is trimmed and normalized to lowercase. New passwords use Argon2id; bcrypt hashes from existing accounts are upgraded after successful signin. JWT access tokens expire after 15 minutes. Refresh tokens expire after 14 days, rotate on use, and are stored as SHA-256 hashes. Reusing an already rotated token revokes its entire family. Signout revokes the current access token immediately. Changing or resetting a password revokes all refresh sessions. Password reset tokens expire after 15 minutes and can be used once. The forgot-password response is the same whether the email exists or not. Public signups receive the user role; create an admin through the CLI.

## Tasks

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/v1/tasks/` | Create with `action` and RFC 3339 `schedule_time` |
| GET | `/api/v1/tasks/` | List own tasks with `page` and `per_page` |
| GET | `/api/v1/tasks/{id}` | Read own task |
| PATCH | `/api/v1/tasks/{id}` | Update optional `action` or `schedule_time` |
| DELETE | `/api/v1/tasks/{id}` | Soft delete, returns 204 |

Pages start at 1; `per_page` defaults to 20 and is capped at 100. List data contains `items` and `meta` with `page`, `per_page`, `total`, and `total_pages`. A task belonging to another user returns 404.

## Files

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/v1/files/` | Upload a raw body with `X-File-Name` and `Content-Type` |
| GET | `/api/v1/files/{id}` | Read own file metadata |
| GET | `/api/v1/files/{id}/download` | Download own file |
| DELETE | `/api/v1/files/{id}` | Soft delete metadata |

Upload size is limited by `MAX_UPLOAD_BYTES`. Deleted file bytes remain in private storage for recovery or scheduled cleanup. Another user's file returns 404.

## Administration and status

Scheduler routes are under `/api/v1/scheduler`; system configuration routes are under `/api/v1/system-config`. Their existing permission requirements remain in force. See [scheduler usage](scheduler_usage.md).

`GET /live` checks the process. `GET /health` and `GET /ready` check PostgreSQL and Redis. `GET /app-info` requires `system_config:read`.
