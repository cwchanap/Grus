# Grus - AI Coding Assistant Instructions

## Project Overview

Grus is a **multiplayer drawing game platform** with a **modular, game-agnostic architecture**. Built with Fresh (Deno), it supports real-time WebSocket communication, persistent state via Deno KV, and pluggable game engines.

## Core Architecture Patterns

### Game Engine Registry Pattern
- **`lib/core/game-registry.ts`** - Central registry for pluggable game types
- **`lib/core/game-engine.ts`** - Abstract base classes; all games implement `GameEngine<TGameState, TSettings, TClientMessage, TServerMessage>`
- **`lib/games/`** - Game implementations (e.g., `drawing/`) register themselves via `import "./games/index.ts"`
- Add new games: implement engine interface → register in `game-registry.ts` → add types to `types/games/`

### WebSocket Message Flow
- **Global handler**: `routes/api/websocket.ts` uses singleton `CoreWebSocketHandler` (persists across HMR)
- **Message routing**: Core messages (join-room, chat) handled directly; game-specific messages delegated to registered engines
- **Type safety**: All messages extend `BaseClientMessage`/`BaseServerMessage` with `type`, `roomId`, `playerId`, `data` structure
- **Connection pools**: `Map<playerId, WebSocketConnection>` + `Map<roomId, Set<playerId>>` for efficient broadcasting

### Database Layer (Deno KV Only)
- **No SQL database**: Uses Deno KV for all persistence (rooms, players, game state, chat)
- **Service pattern**: `lib/database-factory.ts` provides `IAsyncDatabaseService` interface
- **KV implementation**: `lib/db/kv-room-service.ts` and `lib/db/kv-service.ts`
- **Error handling**: All operations return `DatabaseResult<T>` with `success` boolean and optional `error`

### Fresh Islands Architecture
- **`islands/`** - Client-side interactive components (drawing boards, modals, forms)
- **`components/`** - Server-side reusable UI components (buttons, layouts)
- **`routes/`** - File-based routing; API endpoints under `routes/api/`
- **State management**: Islands use Preact signals; WebSocket state synchronized globally

## Development Workflow

### Essential Commands
```bash
deno task start        # Dev server (port 3000, hot reload)
deno task ci          # Full CI pipeline (format, lint, type-check, test)
deno task test:e2e    # Playwright tests (requires server running)
deno task db:inspect  # Examine KV store contents
deno task setup-hooks # Install pre-commit formatting hooks
```

### Testing Strategy
- **Unit tests**: `lib/**/__tests__/*.ts` for core logic
- **Integration tests**: `islands/**/__tests__/*.tsx` for component behavior
- **E2E tests**: `tests/e2e/` with Playwright; cross-browser multiplayer scenarios
- **Run pattern**: `deno test --allow-all --unstable-kv` (KV flag required)

### Type System Conventions
- **Core types**: `types/core/` (room, game, websocket interfaces)
- **Game-specific**: `types/games/[gametype]/` for custom implementations
- **Generic game engine**: `GameEngine<TGameState, TSettings, TClientMessage, TServerMessage>`
- **Message types**: Always extend base message interfaces with proper `type` discrimination

### Code Quality Patterns
- **Formatting**: Deno built-in formatter (2-space indent, 100 char width, semicolons, double quotes)
- **Linting**: Deno linter with Fresh-specific rules; allows `any` types, prohibits `react-no-danger`
- **Pre-commit hooks**: Auto-format staged files via custom `scripts/lint-staged.ts`
- **Import maps**: `deno.json` imports for consistent dependency resolution

## Project-Specific Conventions

### Error Handling
- Database operations return `DatabaseResult<T>` - never throw
- WebSocket errors sent as `{type: "error", roomId, data: {error: string}}`
- Room operations via `RoomManager` class methods - never manipulate KV directly

### State Management
- Game engines receive immutable state, return new state (no mutations)
- WebSocket state stored in `CoreWebSocketHandler.gameStates` Map
- Room cleanup via `room-manager.ts` with automatic inactive room deletion

### Drawing Game Specifics
- **Pixi.js integration**: `islands/games/drawing/DrawingEngine.tsx` wraps Pixi Application
- **Drawing commands**: Immutable `DrawingCommand` objects for stroke replay
- **Global WebSocket**: `(globalThis as any).__gameWebSocket` for islands to send messages

### Mobile Optimization
- Touch-optimized drawing tools in `components/MobileDrawingTools.tsx`
- Responsive design with Tailwind breakpoints
- Playwright mobile testing (`"Pixel 5"`, `"iPhone 12"` configs)

## Common Integration Points

### Adding Game Types
1. Create `lib/games/[gametype]/engine.ts` implementing `GameEngine`
2. Register in `lib/games/index.ts` import statement
3. Add types to `types/games/[gametype].ts`
4. Create islands in `islands/games/[gametype]/` for game UI

### WebSocket Message Handling
- Core messages handled in `CoreWebSocketHandler.handleMessage()`
- Game messages delegated via `gameEngine.handleClientMessage()`
- Always broadcast state updates via `broadcastToRoom()`

### Database Queries
- Use `getAsyncDatabaseService()` from `lib/database-factory.ts`
- Check `result.success` before accessing `result.data`
- Room operations through `RoomManager` abstraction layer

## Deployment
- **Target**: Deno Deploy via `scripts/deploy-deno.sh`
- **Environment**: Set `JWT_SECRET` (32+ chars) for optional auth
- **Build**: `deno task build` generates production bundle
- **Health check**: `GET /api/health` endpoint for monitoring
