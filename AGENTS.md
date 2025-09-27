# Repository Guidelines

## Project Structure & Module Organization
- `routes/` owns Fresh routing and HTTP/WebSocket APIs; game rooms render via `routes/room/[id].tsx`.
- `components/` hosts server-rendered UI; `islands/` keeps interactive Preact widgets (e.g., `core/MainLobby.tsx`); shared logic sits in `lib/` (`core/room-manager.ts`, `auth/`, `db/`).
- Tests live beside features inside `__tests__/`; cross-journey coverage and Playwright specs live in `tests/e2e/`. Assets, scripts, and optional Prisma files sit in `static/`, `scripts/`, and `prisma/`.

## Build, Test, and Development Commands
- `deno task start` boots the dev server with live reload and KV access; `deno task preview` runs the compiled output.
- `deno task build` prepares the deploy artifact; `deno task check` chains format, lint, and type validation.
- `deno task test`, `deno task test:watch`, and `deno task test:e2e[[:headed|:debug]]` cover unit through multiplayer flows; `deno task db:cleanup[:dry-run|:old|:all]` and `deno task test:create-account` maintain KV state.

## Coding Style & Naming Conventions
- Use `deno fmt` (100-character width, two-space indents, semicolons) and `deno lint` with the bundled Fresh rules; CI expects both clean.
- Keep strict TypeScript: annotate exported APIs, prefer `unknown` over `any`, and reuse shared types from `types/`.
- Name Preact components in `PascalCase`, values in `camelCase`, and Fresh routes with descriptive file names (`api/health.ts`, `[id].tsx`); order Tailwind classes layout → spacing → color.

## Testing Guidelines
- Unit and integration specs belong in feature-specific `__tests__` folders with `.test.ts[x]` suffixes and descriptive `describe()` blocks.
- Run `deno task test:coverage` before release branches; upload `coverage.lcov` when adding major features.
- Playwright suites under `tests/e2e/` default to headless; pass `--headed` or `--debug` when chasing race conditions, and verify both "Pixel 5" and "iPhone 12" device configs.
- Seed accounts through `deno task test:create-account` ahead of authentication paths.

## Commit & Pull Request Guidelines
- Follow conventional commits (`feat:`, `refactor:`, `fix:`); keep subjects imperative under ~72 characters and add focused bodies for multi-area changes.
- Avoid grab-bag commits—split gameplay, UI, and infra adjustments for easier review.
- Pull requests should explain intent, call out risky areas, link issues, and list verification (`deno task check`, `deno task test`, screenshots/videos for UI).

## Security & Configuration Tips
- Copy `.env.example` to `.env`, set a >32 character `JWT_SECRET`, and provide `DATABASE_URL` when enabling Prisma-backed auth.
- Never log secrets or raw tokens; rely on helpers in `lib/auth/` and clear KV residue with `deno task db:cleanup --dry-run` before destructive operations.
