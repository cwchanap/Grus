# Grus - Multiplayer Game Platform

A real-time multiplayer game platform with pluggable game engines, built on Fresh (Deno), Deno KV, and WebSockets.

## Quick Start for AI Agents

### Essential Commands (All require `--unstable-kv`)
```bash
deno task start          # Dev server (port 3000) with HMR
deno task ci             # Format, lint, type-check, test (run before commits)
deno task test           # Unit + integration tests
deno task test:e2e       # Playwright E2E tests (headed: :headed, debug: :debug)
deno task db:inspect     # View KV store contents
deno task db:cleanup     # Clean old/empty rooms (--dry-run, --all, --old-rooms)
```

### Core Architecture Principles

**This is NOT just a drawing game** - it's a game-agnostic multiplayer platform. The architecture separates:
- **Core multiplayer logic** (`lib/core/`) - rooms, connections, message routing
- **Pluggable game engines** (`lib/games/`) - game-specific rules and state
- **Game registry** (`lib/core/game-registry.ts`) - runtime game type discovery

### Critical HMR Survival Pattern (WebSocket)

**Problem**: Deno's HMR reloads modules, destroying WebSocket connections.  
**Solution**: Pin handler to `globalThis` in `routes/api/websocket.ts`:

```typescript
function getWebSocketHandler(): CoreWebSocketHandler {
  const g = globalThis as unknown as { __WS_HANDLER__?: CoreWebSocketHandler };
  if (!g.__WS_HANDLER__) g.__WS_HANDLER__ = new CoreWebSocketHandler();
  return g.__WS_HANDLER__; // Survives HMR, connections persist
}
```

**When to use**: Any singleton needing to survive HMR (websockets, connection pools). See also `lib/core/room-manager.ts` for cross-module access pattern.

### Game Engine Integration (Self-Registration Pattern)

**How games work:**
1. **Implement engine**: `lib/games/[type]/engine.ts` extends `BaseGameEngine`
2. **Register in index**: `lib/games/[type]/index.ts` calls `GameRegistry.getInstance().registerGame()`
3. **Import for side effects**: `lib/games/index.ts` imports all game modules
4. **Define types**: `types/games/[type].ts` for messages/state

**Example** (`lib/games/drawing/index.ts`):
```typescript
import { GameRegistry } from "../../core/game-registry.ts";
import { DrawingGameEngine } from "./drawing-engine.ts";

GameRegistry.getInstance().registerGame(
  { id: "drawing", name: "Drawing & Guessing", /* ... */ },
  () => new DrawingGameEngine()
);
```

**Critical**: Import order matters. `lib/games/index.ts` must be imported before using `GameRegistry.getGameEngine()`.

### WebSocket Message Flow

**All messages follow strict typing:**
```typescript
interface BaseClientMessage {
  type: string;      // Message discriminator
  roomId: string;    // Target room
  playerId: string;  // Sender identity
  data: any;         // Payload (typed per message type)
}

interface BaseServerMessage {
  type: string;      // Message discriminator
  roomId: string;    // Target room
  data: any;         // Payload
}
```

**Message routing** (`lib/core/websocket-handler.ts`):
1. Core messages (`join-room`, `leave-room`, `chat`) → handled directly
2. Game messages → delegated to `GameEngine.handleClientMessage()`
3. Server responses → broadcast to room connections

### Database Service Pattern (KV-Only, Result Pattern)

**Single entry point**: `lib/database-factory.ts`
```typescript
const db = getAsyncDatabaseService(); // Returns KVRoomService

// ALL operations return DatabaseResult<T>, NEVER throw
const result = await db.createRoom(name, hostId);
if (!result.success) {
  console.error(result.error); // error is always present when success=false
  return;
}
const roomId = result.data; // data is always present when success=true
```

**Rules**:
- **NEVER** call `Deno.openKv()` directly - use `getAsyncDatabaseService()`
- **NEVER** throw errors - return `{ success: false, error: string }`
- **ALWAYS** use `RoomManager` for room operations, not raw database calls

### Fresh Islands vs Components (Critical Distinction)

**Islands** (`islands/`) - Client-side interactive, bundled for browser:
- Use Preact signals for state: `const [state, setState] = useState()`
- WebSocket connections: `new WebSocket("ws://...")`
- Event handlers: `onClick`, `onSubmit`
- Examples: `islands/core/MainLobby.tsx`, `islands/games/drawing/DrawingBoard.tsx`

**Components** (`components/`) - Server-rendered only, NOT interactive:
- Props-based, no state
- No event handlers (only server actions)
- Styled with Tailwind
- Examples: `components/Button.tsx`, `components/MobileDrawingTools.tsx`

**When to use what**: If it needs client state or responds to user input → Island. If it's static UI → Component.

### Type System Conventions

**Directory structure mirrors functionality**:
- `types/core/` - Platform types (room, game, websocket)
- `types/games/[type]/` - Game-specific types (messages, state)

**Generic game engine signature**:
```typescript
class MyGameEngine extends BaseGameEngine<
  MyGameState,        // Game state structure
  MySettings,         // Configuration
  MyClientMessage,    // Client → Server messages
  MyServerMessage     // Server → Client broadcasts
>
```

**Message discrimination**: All messages have `type: string` for runtime switching. Use type guards or `switch` statements.

### Configuration (`lib/config.ts`)

**Hardcoded settings, NOT env vars** (except JWT_SECRET, DATABASE_URL). Settings include:
- Game limits (max players, rooms, time)
- WebSocket config (heartbeat, timeout)
- Security (rate limits, message length)
- Drawing (batching params)

**Environment detection**: `Deno.env.get("DENO_ENV") === "development"` for dev/prod branching.

### Error Handling Patterns

**Database operations**:
```typescript
const result = await db.operation();
if (!result.success) return result; // Propagate error
// Use result.data safely here
```

**WebSocket errors**:
```typescript
{ type: "error", roomId: string, data: { error: string } }
```

**Never expose internal errors to clients** - sanitize messages in production.

### Drawing Game Implementation Details

**Pixi.js canvas** (`islands/games/drawing/DrawingEngine.tsx`):
- Command pattern: `DrawingCommand` objects for strokes
- Server-side batching: Commands buffered (configurable via `lib/config.ts`)
- Replay: All commands stored for late joiners

**Mobile optimization**: Touch controls in `components/MobileDrawingTools.tsx`, separate from desktop tools.

### Testing Requirements

**All Deno tests need `--unstable-kv`** - configured in `deno.json` tasks.

**E2E tests** (`tests/e2e/`):
- Playwright with device emulation (`"Pixel 5"`, `"iPhone 12"`)
- Multiplayer scenarios (room creation, joining, gameplay)
- Run modes: headless (default), `--headed`, `--debug`

**Test structure**:
- Unit: `lib/**/__tests__/*.test.ts`
- Component: `islands/**/__tests__/*.test.tsx`
- E2E: `tests/e2e/*.test.ts`

### Authentication (Optional)

**Separate concern**: Prisma + PostgreSQL for users, KV for game state.

**Setup**: `deno run -A scripts/setup-prisma.ts` + `deno task test:create-account`

**Key point**: Game works WITHOUT auth (guest mode). Auth adds profiles, not required for core functionality.

### Common Pitfalls

1. **Forgetting `--unstable-kv`** → Tests/dev server fail with KV errors
2. **Direct KV access** → Use `getAsyncDatabaseService()` or `RoomManager`
3. **Throwing errors in DB ops** → Return `DatabaseResult` with `success: false`
4. **Islands without client bundle** → If it's static, move to `components/`
5. **Missing game registration** → Games won't work until imported in `lib/games/index.ts`
6. **Console.log in production** → Remove debug logs before committing

### Key File Reference

| File | Purpose |
|------|---------|
| `routes/api/websocket.ts` | WebSocket endpoint + HMR survival pattern |
| `lib/core/websocket-handler.ts` | Message routing, connection management |
| `lib/core/room-manager.ts` | Room lifecycle (create, join, leave) |
| `lib/core/game-registry.ts` | Game engine registry (singleton) |
| `lib/games/index.ts` | Game module imports (triggers registration) |
| `lib/database-factory.ts` | Database service factory |
| `lib/config.ts` | Application configuration |
| `islands/core/MainLobby.tsx` | Main lobby (room list, create) |
| `routes/room/[id].tsx` | Room view route |
| `types/core/websocket.ts` | Message type definitions |

### Code Style

**Enforced by `deno fmt` + `deno lint`**:
- 100 char line width, 2 space indent, semicolons required
- No unused vars, prefer const, type annotations on exports
- Tailwind class order: layout → spacing → colors
- PascalCase for components, camelCase for values

**Pre-commit hooks**: Run `deno task setup-hooks` to auto-format staged files.
