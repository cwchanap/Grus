---
trigger: model_decision
description: This rules contains a overview structure for the project and development command. 
---

## Development Commands

### Essential Commands
- `deno task start` - Start development server with hot reload on port 8000
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

### Database Management
- `deno task db:inspect` - Inspect SQLite database contents
- `deno task db:cleanup` - Clean up old/empty rooms
- `deno task db:cleanup:dry-run` - Preview cleanup without changes

### Git Hooks
- `deno task setup-hooks` - Install pre-commit hooks for automatic formatting

## Architecture Overview

### Core System Design
This is a **multiplayer drawing game platform** built with a **modular, game-agnostic architecture** using:
- **Fresh (Deno)** - Full-stack web framework with file-based routing
- **WebSocket communication** - Real-time multiplayer interactions
- **SQLite database** - Persistent storage via `lib/db/database-service.ts`
- **Modular game engine** - Extensible system supporting multiple game types

### Key Architectural Components

#### Core Modules (`lib/core/`)
- **`room-manager.ts`** - Game-agnostic room lifecycle (create, join, leave, cleanup)
- **`websocket-handler.ts`** - WebSocket message routing and connection management  
- **`game-engine.ts`** - Abstract base classes for game implementations
- **`game-registry.ts`** - Registry pattern for pluggable game engines

#### Database Layer (`lib/db/`)
- **`database-service.ts`** - SQLite operations for rooms, players, scores
- **`database-factory.ts`** - Service factory and interface definitions
- Uses **local SQLite** in development, **Cloudflare D1** in production via REST API

#### Game-Specific Implementations (`lib/games/`)
- **`drawing/`** - Drawing game implementation with Pixi.js canvas
- Each game type implements the `GameEngine` interface for pluggability

#### Frontend Architecture
- **`islands/`** - Client-side interactive components (Fresh Islands)
- **`components/`** - Server-side reusable UI components  
- **`routes/`** - File-based routing with API endpoints under `routes/api/`

### WebSocket Message Flow
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
2. Implement `GameEngine` interface from `lib/core/game-engine.ts`
3. Register in `lib/core/game-registry.ts`
4. Add type definitions to `types/games/[gametype].ts`

### Testing Strategy  
- **Unit tests** - Core logic and utilities
- **Integration tests** - Component interactions  
- **E2E tests** - Full user workflows with Playwright
- **WebSocket tests** - Real-time communication scenarios

### Code Quality Enforcement
- **Pre-commit hooks** - Automatic formatting and type checking
- **Custom lint-staged** - Process only staged files for performance
- **Deno built-in tools** - Fast formatting, linting, and type checking

## Environment Configuration

### Required Environment Variables
```bash
CLOUDFLARE_ACCOUNT_ID=your-account-id
CLOUDFLARE_API_TOKEN=your-api-token  
DATABASE_ID=d616e1fe-17e6-4320-aba2-393a60167603
KV_NAMESPACE_ID=bea0c6d861e7477fae40b0e9c126ed30
```

### Development vs Production
- **Development** - Uses local SQLite database at `db/game.db`
- **Production** - Uses Cloudflare D1 and KV via REST API
- Database service automatically detects environment via `lib/database-factory.ts`

## Common Patterns

### Error Handling
All database operations return `DatabaseResult<T>` with `success` boolean and optional `error` message.

### Room Management  
Use `RoomManager` class methods - never manipulate database directly for room operations.

### WebSocket Messages
Follow `BaseClientMessage`/`BaseServerMessage` patterns with `type`, `roomId`, and `data` fields.

### Game State Updates
Game engines receive immutable state and return new state - no direct mutations.