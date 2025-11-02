# Repository Guidelines

## Project Structure & Module Organization

Fresh routes live in `routes/`, including HTTP handlers and room screens such as `routes/room/[id].tsx`. Server-rendered UI sits in `components/`, while interactive widgets belong in `islands/`—for example, `islands/core/MainLobby.tsx`. Shared business logic and helpers reside under `lib/` (e.g., `lib/core/room-manager.ts`, `lib/auth/`). Place fast unit specs next to features inside `__tests__/`, keep Playwright journeys inside `tests/e2e/`, and store assets or scripts in `static/` and `scripts/` respectively. Optional Prisma schema files belong in `prisma/`.

## Build, Test, and Development Commands

Run `deno task start` for live-reload development with KV access, or `deno task preview` to inspect the production build locally. Use `deno task build` to generate the deployable artifact, and `deno task check` to chain formatting, linting, and type checks. Execute `deno task test` or `deno task test:watch` for unit coverage, and `deno task test:e2e`, optionally with `:headed` or `:debug`, for multiplayer scenarios. When cleaning KV state, prefer `deno task db:cleanup --dry-run` before running destructive variants.

## Coding Style & Naming Conventions

Format all changes with `deno fmt` (100-character width, two-space indentation, semicolons) and lint via `deno lint`. Maintain strict TypeScript: annotate exports, favor `unknown` instead of `any`, and reuse shared types from `types/`. Name Preact components with PascalCase, values with camelCase, and Tailwind classes in layout → spacing → color order. Fresh routes should use descriptive filenames such as `api/health.ts` or `[id].tsx`.

## Testing Guidelines

Write unit and integration tests alongside features using `.test.ts` or `.test.tsx` files with descriptive `describe()` blocks. Playwright specs under `tests/e2e/` should validate both "Pixel 5" and "iPhone 12" device configs; pass `--headed` or `--debug` when isolating race conditions. Before release branches, run `deno task test:coverage` and upload `coverage.lcov` for significant feature additions. Seed authentication flows via `deno task test:create-account`.

## Commit & Pull Request Guidelines

Follow conventional commits such as `feat:`, `fix:`, or `refactor:` with imperative subjects under ~72 characters. Avoid grab-bag commits—separate gameplay, UI, and infrastructure updates for reviewers. Pull requests should explain intent, call out risky areas, reference issues, and list verification steps (`deno task check`, `deno task test`, screenshots or clips for UI changes).

## Security & Configuration Tips

Copy `.env.example` to `.env`, set a >32 character `JWT_SECRET`, and supply `DATABASE_URL` when enabling Prisma auth. Never log secrets or raw tokens; rely on helpers in `lib/auth/` and scrub credentials before sharing logs. Clear residual KV state with `deno task db:cleanup --dry-run` before deleting data, and rerun without `--dry-run` only after confirming the target keys.
