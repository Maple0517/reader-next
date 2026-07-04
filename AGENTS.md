# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.


## Development Workflow


3. Run the narrowest relevant local checks before pushing. Common checks:
   - Backend: `cargo test` or a focused `cargo test <name>`
   - Frontend: `cd frontend && npm test` / `npm run build` as relevant
   - Formatting/sanity: `git diff --check`


### Rust Backend
```bash
cargo run                    # Dev mode
cargo build --release        # Release build
cargo test                   # All tests
cargo test --lib <test_name> # Single test
```

### Local Dev
- Default to single-port mode: build the frontend, run the Rust server, and open `http://localhost:18080`.
- Do not start the Vite dev server (`5173`) unless the user explicitly asks for frontend hot reload/debugging.
- Default backend port is `18080`; check it before starting: `lsof -i :18080`.
- If `18080` is occupied, use the next available port instead of reclaiming the process blindly.
- Prefer explicit port override for local runs: `SERVER_PORT=18080 cargo run`.

### Frontend
```bash
cd frontend && npm install && npm run dev    # Hot-reload frontend only; keep proxy aligned with SERVER_PORT
cd frontend && npm run build                 # Builds to frontend/dist/
```

## Release Workflow

- Release by pushing the code, creating/pushing the version tag, and confirming the Docker publish workflow has been triggered.
- After the Docker publish workflow is queued or running, do not wait for completion by default. Wait only when the user explicitly needs immediate deploy verification or the workflow has recently been unstable.
- Release notes must summarize the actual service/product changes since the previous release. Inspect the diff, relevant files, and user-facing behavior; do not copy commit messages directly as release details.
- When writing release notes, call out backend/API/schema/storage changes, frontend behavior changes, deployment or Docker-impacting changes, and known risks or follow-up checks.

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
<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->
