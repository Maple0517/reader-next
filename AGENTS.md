# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Commands

### Rust Backend
```bash
cargo run                    # Dev mode
cargo build --release        # Release build
cargo test                   # All tests
cargo test --lib <test_name> # Single test
```

### Local Dev
- Default backend port is `18080`; check it before starting: `lsof -i :18080`.
- If `18080` is occupied, use the next available port instead of reclaiming the process blindly.
- Prefer explicit port override for local runs: `SERVER_PORT=18080 cargo run`.

### Frontend
```bash
cd frontend && npm install && npm run dev    # Dev server
cd frontend && npm run build                 # Builds to frontend/dist/
```

## Configuration

Loaded from `.env` file (via `dotenvy`) or environment variables. Separator is `__` for nested keys. See `.env.example` for all options.

Key settings:
- `SERVER_HOST` / `SERVER_PORT` — default `0.0.0.0:18080`
- `DATABASE_URL` — SQLite path, default `sqlite:storage/reader.db?mode=rwc`
- `WEB_ROOT` — static files path, default `frontend/dist`
- `SECURE` / `SECURE_KEY` — security mode toggle
- `INVITE_CODE` — registration gate
- `USER_LIMIT` / `USER_BOOK_LIMIT` — default 50 / 2000
- `LOG_LEVEL` — default `info`
- `REQUEST_TIMEOUT_SECS` — default 15

## Architecture

Rust implementation of "阅读3.0" — a book source reading API server.

### Module Structure
- `src/api/` — HTTP handlers & routing (axum), routes under `/reader3/*`
- `src/service/` — Business logic (book search, sources, users)
- `src/parser/` — Content extraction engine with rule-based parsing
- `src/crawler/` — HTTP fetching via reqwest
- `src/model/` — Data structures (BookSource, rules)
- `src/storage/` — SQLite (sqlx), file cache (MD5 key), filesystem ops
- `src/app/` — Config, logging, server setup
- `src/error/` — Error types
- `src/util/` — Utilities

### Request Flow
`api/handlers` → `service/` → `crawler/` (fetch) → `parser/rule_engine` (parse with BookSource rules) → JSON response

### Rule Parsing
`RuleEngine` auto-detects parsing mode:
- **CSS selectors** — default for HTML (`.class`, `#id`, `tag`)
- **JSONPath** — auto-detected for JSON (`$.data.list`)
- **XPath** — lines starting with `/` or `./`
- **JavaScript** — `js:` or `@js:` prefix (rquickjs)
- **Regex** — starts with `:`
- Explicit prefixes: `@css:`, `@json:`, `@xpath:`, `@regex:`

### Book Source Format
JSON objects with `bookSourceUrl`, `bookSourceName`, `searchUrl`/`exploreUrl` (with `${key}` placeholders), and `ruleSearch`/`ruleBookInfo`/`ruleToc`/`ruleContent` parsing rules.

## Important Notes

- **Frontend app**: `frontend/` is the Vue 3 + Vite frontend; production static files come from `frontend/dist/`.
- **`/storage/` is gitignored**: Contains user data and SQLite DB.
- **No tests currently**: `cargo test` will pass but there are no test files written yet.
