# Grus - Multiplayer Game Platform

A real-time multiplayer game platform with pluggable game engines, built on Fresh (Deno), Deno KV, and WebSockets.

## Quick Start for AI Agents

### Essential Commands (All require `--unstable-kv`)
```bash
# Development
deno task start          # Dev server (port 3000) with HMR
deno task ci             # Format, lint, type-check, test (run before commits)
deno task build          # Build for production (Fresh compile + manifest)

# Testing
deno task test           # Run all unit + integration tests (36 test files)
deno task test:watch     # Watch mode for TDD workflow
deno task test:coverage  # Generate coverage report (coverage.lcov)
deno task test:e2e       # Playwright E2E tests (headless by default)
deno task test:e2e:headed    # E2E with visible browser (debugging)
deno task test:e2e:debug     # E2E with Playwright inspector

# Database Management
deno task db:inspect     # View KV store contents (rooms, players, sessions)
deno task db:cleanup     # Clean old/empty rooms (safe, excludes active)
deno task db:cleanup:dry-run  # Preview cleanup without changes
deno task db:cleanup:all      # Nuclear option - delete ALL rooms (dev only)

# Code Quality
deno task format         # Auto-format with deno fmt
deno task lint           # Run deno lint
deno task type-check     # TypeScript type validation

# Git Hooks
deno task setup-hooks    # Install pre-commit formatting hook

# Authentication (Optional)
deno run -A scripts/setup-prisma.ts    # Setup Prisma + PostgreSQL
deno task test:create-account          # Create test@example.com account
```

**Critical**: ALL Deno commands MUST include `--unstable-kv` or they will fail with KV errors.

### Core Architecture Principles

**This is NOT just a drawing game** - it's a game-agnostic multiplayer platform. The architecture separates:
- **Core multiplayer logic** (`lib/core/`) - rooms, connections, message routing
- **Pluggable game engines** (`lib/games/`) - game-specific rules and state
- **Game registry** (`lib/core/game-registry.ts`) - runtime game type discovery

**Key architectural decisions:**
1. **No SQL database for game state** - All transient data (rooms, players, game sessions) in Deno KV
2. **Optional PostgreSQL** - Only for authentication/user profiles via Prisma (separate concern)
3. **Singleton WebSocket handler** - Survives HMR for stable development experience
4. **Self-registering game engines** - Import triggers registration, no manual wiring
5. **Command pattern for drawing** - Replay-based synchronization with batching

**Data flow:**
```
Client WebSocket → WebSocketHandler → RoomManager → GameEngine → KV Storage
                         ↓                               ↓
                   Broadcast ←──────── State Update + Messages
```

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

**Game engine lifecycle methods** (all must be implemented):
```typescript
abstract class BaseGameEngine<TGameState, TSettings, TClientMsg, TServerMsg> {
  // Required implementations:
  abstract getGameType(): string;  // "drawing", "poker", etc.
  abstract initializeGame(roomId, players, settings): TGameState;
  abstract handleClientMessage(state, message): { updatedState, serverMessages };
  abstract validateGameAction(state, playerId, action): boolean;
  abstract calculateScore(state, playerId, action): number;
  
  // Default implementations (can override):
  startGame(state): TGameState;      // Sets phase to "playing"
  endGame(state): TGameState;        // Sets phase to "finished"
  updateGameState(state, deltaTime): TGameState;  // Timer updates
  addPlayer(state, player): TGameState;
  removePlayer(state, playerId): TGameState;
}
```

**Adding a new game type** (step-by-step):
1. Create `lib/games/[gametype]/` directory
2. Create `[gametype]-engine.ts` implementing `BaseGameEngine`
3. Create `[gametype]-utils.ts` for game-specific validation
4. Create `types/games/[gametype].ts` for type definitions
5. Create `lib/games/[gametype]/index.ts` with registration call
6. Add `import "./[gametype]/index.ts";` to `lib/games/index.ts`
7. Create island in `islands/games/[gametype]/` for UI
8. Add game rendering logic in `routes/room/[id].tsx`

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

**Command Batching Pattern** (Performance Critical):
```typescript
// lib/games/drawing/drawing-engine.ts - ServerDrawingCommandBuffer
class ServerDrawingCommandBuffer {
  // Batches drawing commands to reduce WebSocket messages
  // Immediate flush for: start, end, clear commands OR buffer full
  // Debounced flush for: move commands (default 16ms)
  
  add(command: DrawingCommand): void {
    this.buffer.push(command);
    if (command.type === "start" || command.type === "end" || 
        this.buffer.length >= maxBatchSize) {
      this.flush(); // Immediate
    } else {
      // Debounce with setTimeout for smooth drawing
    }
  }
}
```

**Why batching matters**: Without it, a single stroke generates 100+ WebSocket messages. With batching, reduces to 5-10 messages per stroke, saving 90% bandwidth.

**Drawing synchronization**:
1. Drawer sends commands to server
2. Server validates (only current drawer can draw)
3. Server batches and broadcasts to other players
4. Late joiners get full command history for replay

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

### Troubleshooting Guide

**Problem**: WebSocket connections drop on file save during development  
**Solution**: This is normal - HMR reloads modules. The `globalThis.__WS_HANDLER__` pattern minimizes disruption, but connections will briefly disconnect.

**Problem**: `Deno.openKv() is not a function` or KV errors  
**Solution**: Add `--unstable-kv` flag to ALL deno commands. Check `deno.json` task definitions.

**Problem**: Game engine not found or `null` returned from `GameRegistry.getGameEngine()`  
**Solution**: Ensure `import "../../lib/games/index.ts"` is called BEFORE using the registry. Check import order in route files.

**Problem**: Database operations hang or timeout  
**Solution**: Check KV instance initialization. Only one KV connection should exist per process. Use `getAsyncDatabaseService()` singleton.

**Problem**: Player not receiving real-time updates  
**Solution**: 
1. Check WebSocket connection in browser DevTools → Network → WS
2. Verify player is in `roomConnections` map (add debug log in `broadcastToRoom`)
3. Check message type matches what client expects
4. Verify client's message handler is registered

**Problem**: Drawing commands not synchronizing  
**Solution**:
1. Check `validateDrawingAction` - only current drawer can send commands
2. Verify buffer is flushing (check `ServerDrawingCommandBuffer.flush()` calls)
3. Check client is subscribed to `draw-update-batch` messages

**Problem**: TypeScript errors after adding new game type  
**Solution**:
1. Ensure types extend `BaseGameState`, `BaseGameSettings`, etc.
2. Check generic constraints match: `GameEngine<TState, TSettings, TClientMsg, TServerMsg>`
3. Run `deno task type-check` to see all errors

**Problem**: E2E tests failing randomly  
**Solution**:
1. Add explicit waits: `await page.waitForSelector(...)`
2. Use `{ state: "visible" }` option for interactive elements
3. Check for race conditions in WebSocket message handling
4. Run with `--headed` flag to observe browser behavior

**Problem**: Room cleanup deleting active rooms  
**Solution**: Use `--dry-run` first. Default cleanup excludes active rooms and recent rooms (< 1 hour old). Check `scripts/cleanup-database.ts` logic.

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

### Deployment

**Deno Deploy** (Recommended):
```bash
# One-time setup
deno install -A --no-check -r -f https://deno.land/x/deploy/deployctl.ts

# Deploy
deno task build           # Fresh build + manifest generation
bash scripts/deploy-deno.sh   # Deploys to grus-multiplayer-drawing-game.deno.dev

# Post-deployment
# Set environment variables in Deno Deploy dashboard:
# - JWT_SECRET (required if using auth)
# - DATABASE_URL (required if using Prisma/PostgreSQL)
# - DENO_ENV=production (optional, auto-detected)
```

**Health Check** after deployment:
```bash
curl https://your-app.deno.dev/api/health

# Expected response:
{
  "status": "ok",
  "timestamp": "2024-01-01T00:00:00.000Z",
  "environment": "production",
  "checks": {
    "database": { "status": "ok", "latency": 123 },
    "kv_storage": { "status": "ok", "latency": 45 },
    "websocket": { "status": "ok" }
  }
}
```

**Fresh Configuration** (`fresh.config.ts`):
- Port: 3000 (dev), auto in production
- Build targets: Chrome 99+, Firefox 99+, Safari 15+
- Tailwind plugin enabled
- No custom middleware in config (see `routes/_middleware.ts`)

**Environment Variables**:
```bash
# .env (local development only, never commit)
JWT_SECRET=your-secret-key-here-minimum-32-chars    # Required for auth
DATABASE_URL=postgresql://[user]:[password]@[host]/[db]?sslmode=require  # Optional
DENO_ENV=development  # Auto-detected, but can override
```

### Advanced Patterns

**State Management Philosophy**:
- **No Redux/Zustand** - Use Preact signals in Islands for local state
- **No global client state** - Each island manages its own state
- **Server is source of truth** - WebSocket messages drive state updates
- **Optimistic updates** - Client updates local state, server confirms

**WebSocket Message Batching**:
```typescript
// Pattern used in drawing game, applicable to other high-frequency updates
private pendingMessages: ServerMessage[] = [];

// Accumulate messages during game tick
handleGameTick() {
  this.pendingMessages.push(message1, message2, ...);
}

// Flush once per frame or on critical events
flushMessages() {
  if (this.pendingMessages.length === 0) return;
  this.broadcast(this.pendingMessages);
  this.pendingMessages = [];
}
```

**Room State Lifecycle**:
```
1. CREATE    → Room created in KV, host player added
2. WAITING   → Players join, settings configured
3. PLAYING   → Game engine active, real-time updates
4. RESULTS   → Scores displayed, can restart
5. FINISHED  → Game complete, room stays active
6. INACTIVE  → No players, scheduled for cleanup (1 hour)
7. DELETED   → Removed from KV by cleanup script
```

**Performance Optimization Tips**:
1. **Drawing**: Batch commands with debounce (16ms default)
2. **Broadcasting**: Filter recipients by room (don't broadcast to all connections)
3. **KV reads**: Cache room/player data in memory, update on changes
4. **Type validation**: Move to build-time with strict TypeScript, avoid runtime checks
5. **Asset loading**: Use Fresh's static/ directory for auto-optimization

**Security Considerations**:
- **Message validation**: Always validate `playerId`, `roomId` before processing
- **Rate limiting**: Configured in `lib/config.ts` (drawing: 100/sec, messages: 60/min)
- **XSS prevention**: Tailwind classes only, no inline styles
- **CSRF**: Not applicable (stateless WebSocket, JWT in cookies)
- **SQL injection**: N/A (Deno KV, no SQL)

**Testing Strategy**:
- **Unit tests** (`lib/**/__tests__/`): Pure functions, game logic, utilities
- **Integration tests** (`lib/core/__tests__/`): Room manager, WebSocket handler
- **Component tests** (`islands/**/__tests__/`): Island rendering, user interactions
- **E2E tests** (`tests/e2e/`): Full user journeys, multiplayer scenarios
- **Coverage goal**: >80% for core logic, >60% overall

**Mobile Considerations**:
- **Touch events**: Handle in separate components (`MobileDrawingTools.tsx`)
- **Viewport**: Use `vh` units, test on real devices
- **Performance**: Reduce particle effects, simplify animations
- **Device testing**: Playwright configs for Pixel 5, iPhone 12
- **Network**: Handle poor connectivity with retry logic

### File Organization Principles

```
routes/              → Fresh routes, HTTP handlers, SSR
  api/              → REST endpoints, WebSocket upgrade
  room/[id].tsx     → Dynamic room pages
  _middleware.ts    → Auth, logging, CORS
  
islands/            → Client-interactive Preact components
  core/             → Platform islands (lobby, chat)
  games/[type]/     → Game-specific UI
  
components/         → Server-rendered UI components
  ui/               → Reusable primitives (Button, Card)
  games/            → Game-specific static UI
  
lib/                → Business logic, no UI
  core/             → Platform logic (rooms, WebSocket)
  games/[type]/     → Game engines, rules
  db/               → Database services (KV)
  auth/             → Authentication (optional)
  config.ts         → Configuration
  
types/              → TypeScript definitions
  core/             → Platform types
  games/[type]/     → Game-specific types
  
scripts/            → Utility scripts (deploy, cleanup)
tests/e2e/          → Playwright tests
static/             → Public assets (images, fonts)
```

**Import Conventions**:
- Use `$fresh/` for Fresh imports (defined in `deno.json` imports)
- Relative imports for local files: `../../lib/core/`
- Absolute imports not supported (no TypeScript paths config)
- Import order: External → Fresh → Local lib → Types → Components/Islands
