# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Essential Commands

- `deno task start` - Start development server with hot reload on port 3000
- `deno task build` - Build for production
- `deno task preview` - Preview production build
- `deno task ci` - Run all CI checks (format, lint, type-check, tests)

### Code Quality

- `deno task format` - Format code using Deno formatter
- `deno task lint` - Run Deno linter
- `deno task type-check` - Run TypeScript type checking
- `deno task check` - Combined format check, lint, and type check

### Testing

- `deno task test` - Run all tests (unit, integration, and e2e)
- `deno task test:watch` - Run tests in watch mode
- `deno task test:coverage` - Run tests with coverage report
- `deno task test:e2e` - Run end-to-end tests with Playwright
- `deno task test:e2e:headed` - Run e2e tests in headed mode for debugging
- `deno task test:create-account` - Create test account for authentication testing

### Authentication (Optional)

- `deno run -A scripts/setup-prisma.ts` - Set up Prisma and database schema
- See `docs/authentication-setup.md` for full authentication setup guide

### Database Management

- `deno task db:inspect` - Inspect Deno KV database contents
- `deno task db:cleanup` - Clean up old/empty rooms
- `deno task db:cleanup:dry-run` - Preview cleanup without changes

### Git Hooks

- `deno task setup-hooks` - Install pre-commit hooks for automatic formatting

## Architecture Overview

### Core System Design

This is a **multiplayer drawing game platform** built with a **modular, game-agnostic architecture** using:

- **Fresh (Deno)** - Full-stack web framework with file-based routing
- **WebSocket communication** - Real-time multiplayer interactions
- **Deno KV** - Persistent storage via `lib/db/kv-room-service.ts` and `lib/db/kv-service.ts`
- **Modular game engine** - Extensible system supporting multiple game types

### Key Architectural Components

#### Core Modules (`lib/core/`)

- **`room-manager.ts`** - Game-agnostic room lifecycle (create, join, leave, cleanup)
- **`websocket-handler.ts`** - WebSocket message routing and connection management
- **`game-engine.ts`** - Abstract base classes for game implementations
- **`game-registry.ts`** - Registry pattern for pluggable game engines

#### Database Layer (`lib/db/`)

- **`kv-room-service.ts`** - Room and player persistence using Deno KV
- **`kv-service.ts`** - Generic KV operations (game state, chat, drawing data)
- **`database-factory.ts`** - KV-backed service factory and interface definitions
- Uses **Deno KV** in all environments

#### Game-Specific Implementations (`lib/games/`)

- **`drawing/`** - Drawing game implementation with Pixi.js canvas
- **`poker/`** - Poker game implementation
- **`index.ts`** - Auto-registers all games via imports (self-registration pattern)
- Each game type implements the `GameEngine` interface for pluggability

#### Frontend Architecture

- **`islands/`** - Client-side interactive components (Fresh Islands)
- **`components/`** - Server-side reusable UI components
- **`routes/`** - File-based routing with API endpoints under `routes/api/`

### WebSocket Message Flow

**Critical**: WebSocket handler survives HMR via `globalThis.__WS_HANDLER__` singleton pattern in `routes/api/websocket.ts`.

1. **Connection** - Client connects via `/api/websocket` route
2. **Room Operations** - Join/leave rooms through `CoreWebSocketHandler`
3. **Game Messages** - Delegated to specific game engines via `GameRegistry`
4. **Real-time Updates** - Broadcast to room participants via connection pools

### Database Schema

- **Rooms** - `id, name, hostId, maxPlayers, gameType, isActive`
- **Players** - `id, name, roomId, isHost, joinedAt`
- **Game Sessions** - Links rooms to game instances
- **Scores** - Per-session scoring with player rankings

### Type System (`types/core/`)

- **`room.ts`** - Room, Player, PlayerState interfaces
- **`game.ts`** - BaseGameState, BaseGameSettings, scoring types
- **`websocket.ts`** - Message type definitions for client/server communication

## Development Workflow

### Adding New Game Types

1. Create game engine in `lib/games/[gametype]/`
   - `engine.ts` - Implements `GameEngine<TState, TSettings, TClientMessage, TServerMessage>`
   - `utils.ts` - Game-specific utilities
   - `index.ts` - Self-registration with `GameRegistry.getInstance().registerGame()`
2. Add type definitions to `types/games/[gametype].ts`
3. Import in `lib/games/index.ts` to trigger auto-registration
4. Create client-side island in `islands/games/[gametype]/`

## Testing Strategy

### Unit and Integration Tests

- **Unit tests** - Core logic and utilities
- **Integration tests** - Component interactions
- **E2E tests** - Full user workflows with Playwright
- **WebSocket tests** - Real-time communication scenarios

### Test Account Setup

The application supports **optional authentication** for enhanced features:

#### Without Authentication (Default)

- Users can play as guests
- No registration required
- Core multiplayer functionality available
- Username navigation limited (no profile page)

#### With Authentication (Optional Setup)

1. **Setup Requirements**:
   ```bash
   # Set up database (see docs/authentication-setup.md)
   deno run -A scripts/setup-prisma.ts

   # Create test account
   deno task test:create-account
   ```

2. **Test Account Credentials** (once authentication is set up):
   ```
   Email:    test@example.com
   Username: testuser
   Password: testpass123
   Name:     Test User
   ```

3. **Testing Authentication Features**:
   - Login/logout functionality
   - Username → profile navigation
   - Persistent user sessions
   - Secure cookie handling

#### Authentication Architecture Notes

- **Database**: Uses Prisma with PostgreSQL (separate from Deno KV game data)
- **Sessions**: JWT-based with HttpOnly cookies
- **Security**: Password hashing, CSRF protection, automatic session cleanup
- **UI Integration**: Login status displayed in main lobby
- **Profile Page**: Accessible via clicking username when authenticated

### Code Quality Enforcement

- **Pre-commit hooks** - Automatic formatting and type checking
- **Custom lint-staged** - Process only staged files for performance
- **Deno built-in tools** - Fast formatting, linting, and type checking

### Critical Development Notes

#### WebSocket Handler Persistence

The WebSocket handler is pinned to `globalThis.__WS_HANDLER__` to survive Deno's HMR (hot module reloading) in development. This prevents connection loss during file changes.

#### Test Requirements

All tests **must** include `--unstable-kv` flag for Deno KV access. The `deno task test` command already includes this.

## Environment Configuration

### Required Environment Variables

- Core game requires no external DB configuration (uses Deno KV).
- For optional authentication, see `docs/authentication-setup.md`.

### Development vs Production

- **Development** - Uses Deno KV
- **Production** - Uses Deno KV
- Service access via `lib/database-factory.ts`

## Common Patterns

### Error Handling

All database operations return `DatabaseResult<T>` with `success` boolean and optional `error` message.

### Room Management

Use `RoomManager` class methods - never manipulate database directly for room operations.

### WebSocket Messages

Follow `BaseClientMessage`/`BaseServerMessage` patterns with `type`, `roomId`, and `data` fields.

### Game State Updates

Game engines receive immutable state and return new state - no direct mutations.
