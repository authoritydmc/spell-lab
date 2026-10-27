SHELL := /bin/bash
CARGO ?= cargo
NPM ?= npm
WEB_DIR := web
PUBLIC_URL ?= http://127.0.0.1:8080

.PHONY: help check test fmt build build-web build-all dev dev-api dev-web \
	docker-up docker-down docker-logs smoke clean

help:
	@printf '%s\n' \
	  'make check       cargo check + frontend typecheck' \
	  'make test        Rust unit tests' \
	  'make fmt         cargo fmt + frontend build' \
	  'make build       release Rust binary' \
	  'make build-web   production React bundle' \
	  'make dev         API :8080 + Vite :5173 (two terminals recommended)' \
	  'make docker-up   docker compose up --build' \
	  'make smoke       end-to-end smoke test (PUBLIC_URL=...)' \
	  'make clean       remove build artefacts'

check:
	$(CARGO) check
	cd $(WEB_DIR) && $(NPM) run typecheck

test:
	$(CARGO) test

fmt:
	$(CARGO) fmt
	cd $(WEB_DIR) && $(NPM) run typecheck

build:
	$(CARGO) build --release --locked

build-web:
	cd $(WEB_DIR) && $(NPM) ci --no-audit --no-fund && $(NPM) run build

build-all: build build-web

dev:
	@echo 'Run these in two terminals:'
	@echo '  make dev-api'
	@echo '  make dev-web'

dev-api:
	SPELL_BIND=127.0.0.1:8080 SPELL_DB_PATH=./data/spell.db RUST_LOG=info,tower_http=info $(CARGO) run

dev-web:
	cd $(WEB_DIR) && $(NPM) run dev

docker-up:
	docker compose up -d --build

docker-down:
	docker compose down

docker-logs:
	docker compose logs -f --tail=100

smoke:
	BASE_URL=$(PUBLIC_URL) bash scripts/smoke.sh

clean:
	$(CARGO) clean
	rm -rf $(WEB_DIR)/dist
