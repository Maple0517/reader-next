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

## AI Book V4 Reference Docs

When working on AI 资料 / AI Book V4, read the relevant architecture guide before changing code:

- Backend V4 pipeline, five-layer boundaries, reducers, projection, quality/correction constraints: `docs/architecture/ai-book-v4-backend.md`
- Frontend V4 console, panels, API/types, Vue component boundaries, display rules: `docs/architecture/ai-book-v4-frontend.md`

Use these docs when touching `src/service/v4/**`, `src/api/handlers/v4.rs`, `frontend/src/views/AiBookV4View.vue`, `frontend/src/components/reader/V4*`, `frontend/src/components/reader/v4/**`, `frontend/src/api/v4/**`, or `frontend/src/types/v4.ts`. Keep V4 work out of V3 unless explicitly requested.

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

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **reader-next** (10521 symbols, 26912 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> Index stale? Run `node .gitnexus/run.cjs analyze` from the project root — it auto-selects an available runner. No `.gitnexus/run.cjs` yet? `npx gitnexus analyze` (npm 11 crash → `npm i -g gitnexus`; #1939).

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "main"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({search_query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.
- For security review, `explain({target: "fileOrSymbol"})` lists taint findings (source→sink flows; needs `analyze --pdg`).

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/reader-next/context` | Codebase overview, check index freshness |
| `gitnexus://repo/reader-next/clusters` | All functional areas |
| `gitnexus://repo/reader-next/processes` | All execution flows |
| `gitnexus://repo/reader-next/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
