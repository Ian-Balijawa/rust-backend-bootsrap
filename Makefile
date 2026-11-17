.DEFAULT_GOAL := help
.PHONY: help up down db-shell db-url migrate run dev check fmt test test-integration verify

help:
	@printf '%s\n' \
		'make up                Start PostgreSQL and Redis containers' \
		'make down              Stop containers' \
		'make db-shell          Open interactive psql shell in PostgreSQL container' \
		'make db-url            Print local PostgreSQL connection URI' \
		'make migrate           Apply database migrations and RBAC seed' \
		'make run               Start the API' \
		'make dev               Start up and API' \
		'make check             Type-check the project' \
		'make fmt               Format Rust files' \
		'make test              Run unit tests' \
		'make test-integration  Run tests using TEST_DATABASE_URL and TEST_REDIS_URL' \
		'make verify            Format, Clippy, and unit tests'

up:
	docker compose up -d --wait

down:
	docker compose down

db-shell:
	docker compose exec postgres psql -U bootstrap -d axum_template

db-url:
	@echo "postgresql://bootstrap:bootstrap@127.0.0.1:$${POSTGRES_HOST_PORT:-15432}/axum_template"

migrate:
	cargo run -- migrate

run:
	cargo run

dev: up migrate
	cargo run

check:
	cargo check

fmt:
	cargo fmt

test:
	cargo test

test-integration:
	@test -n "$(TEST_DATABASE_URL)" || (echo 'TEST_DATABASE_URL is required' >&2; exit 1)
	@test -n "$(TEST_REDIS_URL)" || (echo 'TEST_REDIS_URL is required' >&2; exit 1)
	cargo test -- --ignored

verify:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test
