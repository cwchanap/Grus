# Grus Architecture Documentation

This document provides a deep dive into the Grus multiplayer game platform architecture.

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            CLIENT LAYER                                  │
├─────────────────────────────────────────────────────────────────────────┤
│  Browser                                                                 │
│  ┌────────────┐  ┌──────────────┐  ┌───────────────┐                  │
│  │  Islands   │  │  Components  │  │  Static Pages │                  │
│  │ (Client-   │  │  (Server-    │  │  (SSR Routes) │                  │
│  │  side JS)  │  │   rendered)  │  │               │                  │
│  └─────┬──────┘  └──────────────┘  └───────────────┘                  │
│        │                                                                 │
│        │ WebSocket                                                       │
│        │ Connection                                                      │
└────────┼─────────────────────────────────────────────────────────────────┘
         │
         │
┌────────┼─────────────────────────────────────────────────────────────────┐
│        │                     SERVER LAYER                                │
├────────┼─────────────────────────────────────────────────────────────────┤
│        ↓                                                                  │
│  ┌──────────────────┐         routes/                                   │
│  │ WebSocket        │         ├── api/websocket.ts ← Entry point        │
│  │ Handler          │         ├── room/[id].tsx                         │
│  │ (globalThis      │         └── _middleware.ts                        │
│  │  singleton)      │                                                    │
│  └────────┬─────────┘                                                    │
│           │                                                               │
│           ↓                                                               │
│  ┌──────────────────┐         lib/core/                                 │
│  │ Room Manager     │         ├── room-manager.ts                       │
│  │                  │         ├── websocket-handler.ts                  │
│  └────────┬─────────┘         ├── game-registry.ts                      │
│           │                   └── game-engine.ts (abstract)             │
│           │                                                               │
│           ↓                                                               │
│  ┌──────────────────┐         lib/games/                                │
│  │ Game Engine      │         ├── drawing/                              │
│  │ (Drawing/Poker)  │         │   ├── drawing-engine.ts                 │
│  └────────┬─────────┘         │   └── index.ts (registers)              │
│           │                   └── poker/                                 │
│           │                       └── poker-engine.ts                    │
│           ↓                                                               │
│  ┌──────────────────┐         lib/db/                                   │
│  │ Database         │         ├── kv-room-service.ts                    │
│  │ Service          │         └── kv-service.ts                         │
│  │ (KV wrapper)     │                                                    │
│  └────────┬─────────┘                                                    │
│           │                                                               │
└───────────┼───────────────────────────────────────────────────────────────┘
            │
            ↓
┌───────────────────────────────────────────────────────────────────────────┐
│                         STORAGE LAYER                                     │
├───────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐              ┌────────────────────┐                │
│  │   Deno KV       │              │   PostgreSQL       │                │
│  │   (Primary)     │              │   (Optional Auth)  │                │
│  │                 │              │                     │                │
│  │ • Rooms         │              │ • Users            │                │
│  │ • Players       │              │ • Sessions         │                │
│  │ • Game State    │              │ • Profiles         │                │
│  │ • Chat Messages │              │                     │                │
│  │ • Scores        │              │   via Prisma       │                │
│  └─────────────────┘              └────────────────────┘                │
└───────────────────────────────────────────────────────────────────────────┘
```

## Message Flow

### Client → Server (Player Action)

```
┌──────────┐     WebSocket      ┌────────────────┐
│  Island  │ ─────message──────> │  WS Handler    │
│          │   {type, roomId,    │  (singleton)   │
│          │    playerId, data}  │                │
└──────────┘                     └────────┬───────┘
                                          │
                                          ↓
                                 ┌────────────────┐
                                 │ Room Manager   │
                                 │ getRoomState() │
                                 └────────┬───────┘
                                          │
                                          ↓
                                 ┌────────────────┐
                                 │ Game Engine    │
                                 │ handleClient   │
                                 │ Message()      │
                                 └────────┬───────┘
                                          │
                                          ↓
                              ┌──────────────────────┐
                              │  Returns:            │
                              │  • updatedState      │
                              │  • serverMessages[]  │
                              └──────────┬───────────┘
                                         │
                                         ↓
                              ┌──────────────────────┐
                              │ KV Service           │
                              │ saveGameState()      │
                              └──────────────────────┘
```

### Server → Clients (Broadcast)

```
┌────────────────┐
│ Game Engine    │ Returns serverMessages[]
└────────┬───────┘
         │
         ↓
┌────────────────┐
│ WS Handler     │ broadcastToRoom(roomId, messages)
└────────┬───────┘
         │
         ├─────────────────┬──────────────┬──────────────┐
         ↓                 ↓              ↓              ↓
    ┌────────┐        ┌────────┐    ┌────────┐    ┌────────┐
    │Player 1│        │Player 2│    │Player 3│    │Player N│
    │ Island │        │ Island │    │ Island │    │ Island │
    └────────┘        └────────┘    └────────┘    └────────┘
         │                 │              │              │
         └─────────────────┴──────────────┴──────────────┘
                    All update local state
```

## Game Engine Architecture

### Base Game Engine Contract

```typescript
interface GameEngine<TGameState, TSettings, TClientMsg, TServerMsg> {
  // Lifecycle
  initializeGame(roomId, players, settings): TGameState
  startGame(gameState): TGameState
  endGame(gameState): TGameState
  
  // Core loop
  handleClientMessage(gameState, message): {
    updatedState: TGameState,
    serverMessages: TServerMsg[]
  }
  updateGameState(gameState, deltaTime): TGameState
  
  // Validation
  validateGameAction(gameState, playerId, action): boolean
  
  // Player management
  addPlayer(gameState, player): TGameState
  removePlayer(gameState, playerId): TGameState
  
  // Scoring
  calculateScore(gameState, playerId, action): number
}
```

### Drawing Game Engine Implementation

```
DrawingGameEngine extends BaseGameEngine
├── ServerDrawingCommandBuffer (batching)
│   ├── add(command): void
│   ├── flush(): void
│   └── destroy(): void
│
├── initializeGame()
│   └── Creates empty canvas state
│
├── handleClientMessage()
│   ├── "draw" → validate → buffer → batch broadcast
│   ├── "guess" → check answer → update scores
│   ├── "clear" → clear canvas → broadcast
│   └── "skip-word" → next word → broadcast
│
├── validateDrawingAction()
│   ├── Check: is current drawer?
│   ├── Check: is playing phase?
│   └── Check: valid command structure?
│
└── calculateScore()
    └── Points based on guess timing
```

## State Management

### Room State Lifecycle

```
CREATE ──> WAITING ──> PLAYING ──> RESULTS ──> FINISHED
  │          │           │           │           │
  │          │           │           │           └─> Can restart (→ WAITING)
  │          │           │           └───> Display scores
  │          │           └───> Game engine active
  │          └───> Players join, configure settings
  └───> Room created, host assigned

After 1 hour of inactivity (no players): FINISHED → INACTIVE → DELETED
```

### State Synchronization Strategy

1. **Server as Source of Truth**
   - All state mutations happen server-side
   - Clients send actions, not state updates

2. **Optimistic Updates** (Drawing only)
   - Drawer sees immediate feedback
   - Server validates and broadcasts to others

3. **State Reconciliation**
   - Late joiners receive full state snapshot
   - Incremental updates for connected players

## WebSocket Connection Management

### Connection Pools

```typescript
class CoreWebSocketHandler {
  // Connection tracking
  private connections: Map<connectionId, WebSocketConnection>
  private roomConnections: Map<roomId, Set<connectionId>>
  
  // Game state (in-memory)
  private gameStates: Map<roomId, BaseGameState>
  
  // Singleton pattern for HMR survival
  static getInstance(): CoreWebSocketHandler {
    if (!globalThis.__WS_HANDLER__) {
      globalThis.__WS_HANDLER__ = new CoreWebSocketHandler()
    }
    return globalThis.__WS_HANDLER__
  }
}
```

### Message Validation Flow

```
Incoming message
      │
      ↓
┌──────────────┐
│ Parse JSON   │ → Invalid JSON → Send error → Close
└──────┬───────┘
       │
       ↓
┌──────────────────┐
│ Validate schema  │ → Missing fields → Send error
│ (type, roomId,   │
│  playerId, data) │
└──────┬───────────┘
       │
       ↓
┌──────────────────┐
│ Verify playerId  │ → Not in room → Send error
│ in roomId        │
└──────┬───────────┘
       │
       ↓
┌──────────────────┐
│ Route to handler │
│ (core or game)   │
└──────────────────┘
```

## Performance Optimizations

### 1. Command Batching (Drawing)

**Problem**: Single stroke = 100+ WebSocket messages  
**Solution**: Batch commands, flush every 16ms or when buffer full

```typescript
Before batching:  100 messages/stroke × 5 players = 500 broadcasts
After batching:   5-10 messages/stroke × 5 players = 25-50 broadcasts
Bandwidth saved: 90%
```

### 2. Room-Scoped Broadcasting

**Problem**: Broadcasting to all connections scales O(n)  
**Solution**: Only send to players in target room

```typescript
// Bad: O(n) where n = all connections
for (const [_, conn] of this.connections) {
  conn.ws.send(message)
}

// Good: O(m) where m = players in room
const roomConns = this.roomConnections.get(roomId) || new Set()
for (const connId of roomConns) {
  const conn = this.connections.get(connId)
  if (conn) conn.ws.send(message)
}
```

### 3. KV Caching Strategy

```typescript
// Cache frequently accessed data in memory
private roomCache: Map<roomId, Room> = new Map()

async getRoom(roomId: string): Promise<Room> {
  // Check cache first
  if (this.roomCache.has(roomId)) {
    return this.roomCache.get(roomId)
  }
  
  // Fallback to KV
  const room = await kv.get(["rooms", roomId])
  if (room) this.roomCache.set(roomId, room)
  return room
}

// Invalidate cache on updates
async updateRoom(roomId: string, updates: Partial<Room>) {
  await kv.set(["rooms", roomId], updates)
  this.roomCache.delete(roomId) // Force refresh next read
}
```

## Security Architecture

### Rate Limiting

```typescript
// lib/config.ts
security: {
  rateLimitMessages: 60,    // messages per minute per player
  rateLimitDrawing: 100,    // drawing commands per second
  maxMessageLength: 500,    // characters
  maxPlayerNameLength: 20,  // characters
}

// Implementation
class RateLimiter {
  private messageCounts: Map<playerId, number[]> = new Map()
  
  checkLimit(playerId: string, limit: number, windowMs: number): boolean {
    const now = Date.now()
    const counts = this.messageCounts.get(playerId) || []
    
    // Remove old timestamps outside window
    const recentCounts = counts.filter(t => t > now - windowMs)
    
    if (recentCounts.length >= limit) {
      return false // Rate limit exceeded
    }
    
    recentCounts.push(now)
    this.messageCounts.set(playerId, recentCounts)
    return true
  }
}
```

### Message Validation

```typescript
// All incoming messages validated against schemas
function validateClientMessage(msg: unknown): msg is BaseClientMessage {
  return (
    typeof msg === "object" &&
    msg !== null &&
    "type" in msg && typeof msg.type === "string" &&
    "roomId" in msg && typeof msg.roomId === "string" &&
    "playerId" in msg && typeof msg.playerId === "string" &&
    "data" in msg
  )
}

// Game-specific validation
function validateDrawingCommand(cmd: unknown): cmd is DrawingCommand {
  if (typeof cmd !== "object" || cmd === null) return false
  
  const { type, x, y, color, brushSize } = cmd as any
  
  return (
    ["start", "move", "end", "clear"].includes(type) &&
    (type === "clear" || (
      typeof x === "number" && x >= 0 && x <= 800 &&
      typeof y === "number" && y >= 0 && y <= 600 &&
      typeof color === "string" && /^#[0-9A-Fa-f]{6}$/.test(color) &&
      typeof brushSize === "number" && brushSize >= 1 && brushSize <= 50
    ))
  )
}
```

## Testing Architecture

### Test Pyramid

```
        ┌─────┐
       /       \
      /   E2E   \        12 tests (Playwright)
     /───────────\       • Full user journeys
    /             \      • Multiplayer scenarios
   /  Integration  \     • Cross-browser testing
  /─────────────────\
 /                   \   18 tests
/   Unit Tests       \  • Game engines
──────────────────────  • Utilities
                        • Validation
                        • 36 total test files
```

### Test Coverage Strategy

- **Core logic**: >90% coverage required
  - `lib/core/room-manager.ts`
  - `lib/games/*/engine.ts`
  - `lib/db/kv-room-service.ts`

- **Integration**: >80% coverage target
  - WebSocket handler
  - Message routing
  - State synchronization

- **UI/Islands**: >60% coverage goal
  - User interactions
  - State updates
  - Error handling

## Deployment Architecture

### Deno Deploy

```
GitHub Repository
      │
      │ git push
      ↓
┌──────────────────┐
│ Deno Deploy      │
│ (Edge Runtime)   │
├──────────────────┤
│ • Auto-scaling   │
│ • Global CDN     │
│ • KV replication │
│ • Zero-downtime  │
└────────┬─────────┘
         │
         ├─────> Deno KV (Foundationdb)
         ├─────> Static assets (CDN)
         └─────> PostgreSQL (optional, Neon)
```

### Environment Configuration

```
Development:
- Local Deno KV (file-based)
- Hot module reloading
- Debug logging enabled
- CORS permissive

Production:
- Distributed Deno KV
- No HMR
- Error logging only
- CORS restricted
```

## Monitoring & Observability

### Health Check Endpoint

```typescript
// routes/api/health.ts
GET /api/health

Response:
{
  status: "ok" | "degraded" | "down",
  timestamp: ISO8601,
  environment: "development" | "production",
  checks: {
    database: { status: "ok", latency: 123 },
    kv_storage: { status: "ok", latency: 45 },
    websocket: { status: "ok" }
  }
}
```

### Metrics to Monitor

1. **WebSocket Connections**
   - Active connections count
   - Connection duration
   - Reconnection rate

2. **Game Performance**
   - Rooms active/inactive
   - Players per room average
   - Message latency (p50, p99)

3. **Database**
   - KV operation latency
   - Read/write ratio
   - Storage usage

4. **Errors**
   - WebSocket disconnects
   - Message validation failures
   - Game engine errors

## Future Architecture Considerations

### Scalability

**Current limits** (single Deno Deploy instance):
- ~1000 concurrent WebSocket connections
- ~100 active game rooms
- ~800 players simultaneous

**Scaling strategies**:
1. Horizontal scaling with room sharding
2. Redis Pub/Sub for cross-instance messaging
3. Dedicated game server instances
4. Read replicas for KV

### Feature Extensions

**Potential additions**:
- Voice chat (WebRTC)
- Replays/recordings (store command history)
- Spectator mode (read-only WebSocket)
- Tournament system (bracket management)
- Achievements (persistent in PostgreSQL)

### Technology Alternatives

**If requirements change**:
- **Socket.io** instead of native WebSocket (more features, heavier)
- **Redis** instead of Deno KV (more mature, requires separate service)
- **tRPC** instead of raw WebSocket (type-safe RPC, less flexible)
- **SvelteKit/Next.js** instead of Fresh (larger ecosystem, different deployment)
