# Grus - Multiplayer Drawing Game Platform

## Quick Start for AI Agents

Essential knowledge for being productive in this codebase immediately:

### Core Architecture (Game-Agnostic Platform)
- **Multiplayer game platform** with pluggable game engines, not just a drawing game
- **Fresh (Deno)** full-stack framework with file-based routing and Islands architecture
- **Deno KV** for all persistence (rooms, players, game state) - no SQL database
- **WebSocket** real-time communication with singleton handler surviving HMR

### Critical Development Commands
```bash
deno task start          # Dev server (port 3000) - REQUIRES --unstable-kv flag
deno task ci            # Complete CI pipeline (format, lint, type-check, test)
deno test --allow-all --unstable-kv  # All tests MUST include --unstable-kv
deno task db:inspect    # Debug KV store contents
deno task test:e2e      # Playwright e2e tests
```

### Game Engine Registry Pattern (Core Architecture)
```typescript
// lib/core/game-registry.ts - Singleton registry
GameRegistry.getInstance().registerGame(gameInfo, engineFactory)

// lib/games/index.ts - Auto-registers all games via imports
import "./drawing/index.ts"; // Self-registration pattern

// New game integration:
// 1. lib/games/[type]/engine.ts implements GameEngine<TState, TSettings, TClient, TServer>
// 2. lib/games/[type]/index.ts registers with GameRegistry
// 3. types/games/[type].ts defines message/state types
```

### WebSocket Message Flow (Critical for Multiplayer)
```typescript
// routes/api/websocket.ts - Global handler persists across HMR
function getWebSocketHandler(): CoreWebSocketHandler {
  const g = globalThis as unknown as { __WS_HANDLER__?: CoreWebSocketHandler };
  if (!g.__WS_HANDLER__) g.__WS_HANDLER__ = new CoreWebSocketHandler();
  return g.__WS_HANDLER__;
}

// Message structure - ALL messages must follow:
interface BaseClientMessage {
  type: string; roomId: string; playerId: string; data: any;
}

// Core messages handled directly, game-specific delegated to engines
```

### Fresh Islands vs Components Pattern
- **`islands/`** - Client-side interactive (Preact signals, WebSocket connections)
- **`components/`** - Server-side reusable UI (no client state)  
- **Islands** handle user interactions, modals, drawing boards
- **Components** handle layouts, buttons, server-rendered content

### Database Service Pattern (KV-Only)
```typescript
// lib/database-factory.ts - Single entry point
const db = getAsyncDatabaseService(); // Returns KV-backed service

// All operations return DatabaseResult<T>
const result = await db.createRoom(name, hostId);
if (!result.success) { /* handle result.error */ }

// NEVER manipulate KV directly - always through RoomManager or services
```

### Type System Conventions
- **Core types**: `types/core/` (room.ts, game.ts, websocket.ts)
- **Game types**: `types/games/[type]/` for game-specific messages/state
- **Generic engines**: `GameEngine<TGameState, TSettings, TClientMessage, TServerMessage>`
- **Message discrimination**: All messages have `type` field for runtime switching

### Error Handling Patterns
```typescript
// Database operations - never throw, always return result
DatabaseResult<T> = { success: boolean; data?: T; error?: string }

// WebSocket errors - structured message format  
{ type: "error", roomId: string, data: { error: string } }

// Room operations - always through RoomManager, never direct KV manipulation
```

### Drawing Game Specifics (Primary Implementation)
- **Pixi.js** integration in `islands/games/drawing/DrawingEngine.tsx`
- **Command pattern**: `DrawingCommand` objects for stroke replay and sync
- **Server-side batching**: Commands buffered and flushed for performance
- **Touch optimization**: Mobile tools in `components/MobileDrawingTools.tsx`

### Key File Locations
- **WebSocket entry**: `routes/api/websocket.ts` 
- **Room logic**: `lib/core/room-manager.ts`
- **Game registry**: `lib/core/game-registry.ts` + `lib/games/index.ts`
- **Database abstraction**: `lib/database-factory.ts`
- **Main lobby**: `islands/core/MainLobby.tsx`
- **Room view**: `routes/room/[id].tsx`

### Testing Requirements
- **All tests** need `--unstable-kv` flag for Deno KV
- **E2E tests** use Playwright with multiplayer scenarios
- **Unit tests** in `lib/**/__tests__/`, **component tests** in `islands/**/__tests__/`
- **Mobile testing** includes `"Pixel 5"` and `"iPhone 12"` configs

### Authentication (Optional Feature)
- **Prisma + PostgreSQL** for user data (separate from KV game state)
- **KV handles** all game/room state regardless of auth status
- **Test accounts** available via `deno task test:create-account`
- **Profile navigation** - username clickable in `MainLobby.tsx`
