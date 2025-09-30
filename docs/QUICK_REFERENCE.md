# Grus Quick Reference Cheat Sheet

Quick lookup for common tasks and patterns when working with Grus.

## 🚀 Common Commands

```bash
# Development
deno task start                    # Start dev server
deno task start --port 8000        # Custom port (won't work - see fresh.config.ts)

# Testing
deno task test                     # All tests
deno task test:watch               # Watch mode
deno task test:e2e:headed          # Visual E2E debugging

# Quality
deno task ci                       # Run all checks before commit

# Database
deno task db:inspect               # View KV contents
deno task db:cleanup:dry-run       # Safe preview before cleanup
```

## 📁 Where to Put New Code

| I need to add... | Put it in... | Example |
|-----------------|--------------|---------|
| A new game type | `lib/games/[type]/` | `lib/games/chess/chess-engine.ts` |
| Game UI | `islands/games/[type]/` | `islands/games/chess/ChessBoard.tsx` |
| Reusable UI | `components/ui/` | `components/ui/Dialog.tsx` |
| Type definitions | `types/games/[type].ts` | `types/games/chess.ts` |
| Utility functions | `lib/games/[type]/utils.ts` | `lib/games/chess/chess-utils.ts` |
| API endpoint | `routes/api/` | `routes/api/leaderboard.ts` |
| Page route | `routes/` | `routes/leaderboard.tsx` |
| Test file | Same dir + `__tests__/` | `lib/games/chess/__tests__/chess-engine.test.ts` |

## 🎮 Adding a New Game (Checklist)

- [ ] Create `lib/games/[gametype]/` directory
- [ ] Create `[gametype]-engine.ts` extending `BaseGameEngine`
- [ ] Create `types/games/[gametype].ts` for state/messages
- [ ] Create `[gametype]-utils.ts` for validation
- [ ] Create `lib/games/[gametype]/index.ts` with registry call:
  ```typescript
  GameRegistry.getInstance().registerGame(gameInfo, () => new MyGameEngine())
  ```
- [ ] Add import to `lib/games/index.ts`: `import "./[gametype]/index.ts";`
- [ ] Create UI island: `islands/games/[gametype]/GameBoard.tsx`
- [ ] Add game rendering in `routes/room/[id].tsx`
- [ ] Write tests: `lib/games/[gametype]/__tests__/`
- [ ] Update docs: Add game to README

## 🔧 Common Patterns

### Database Operation
```typescript
// ✅ Always use the pattern
const db = getAsyncDatabaseService();
const result = await db.createRoom(name, hostId);
if (!result.success) {
  // Handle error: result.error
  return;
}
const roomId = result.data; // Safe to use

// ❌ Never do this
const kv = await Deno.openKv(); // Don't access KV directly
```

### WebSocket Message
```typescript
// Send from client (Island)
ws.send(JSON.stringify({
  type: "game-action",
  roomId: roomId,
  playerId: playerId,
  data: { action: "fold" }
}));

// Handle on server (GameEngine)
handleClientMessage(gameState, message) {
  switch (message.type) {
    case "game-action":
      // Validate, update state, return messages
      return {
        updatedState: { ...gameState, /* changes */ },
        serverMessages: [{ type: "action-result", roomId, data }]
      };
  }
}
```

### Component vs Island
```typescript
// ✅ Island - Has client state/interactions
export default function DrawingBoard({ roomId }: Props) {
  const [isDrawing, setIsDrawing] = useState(false); // Client state
  const handleClick = () => { /* ... */ }; // Event handler
  return <canvas onClick={handleClick} />;
}

// ✅ Component - Static UI only
export function Button({ children, onClick }: Props) {
  return <button type="button" onClick={onClick}>{children}</button>;
}
// No useState, no WebSocket, just props in → JSX out
```

## 🐛 Debugging Checklist

**WebSocket not connecting?**
1. Check browser DevTools → Network → WS tab
2. Verify `routes/api/websocket.ts` is running
3. Check `globalThis.__WS_HANDLER__` exists in console
4. Look for connection errors in server logs

**Player not receiving updates?**
1. Check `roomConnections.get(roomId)` includes player
2. Verify message type matches client handler
3. Add debug log in `broadcastToRoom`
4. Check if WebSocket readyState === OPEN

**Game engine not found?**
1. Ensure `lib/games/index.ts` imports your game
2. Check registry: `GameRegistry.getInstance().getGameEngine("yourtype")`
3. Verify import order (games index before route)

**KV errors?**
1. Add `--unstable-kv` to command
2. Check KV instance initialization
3. Verify not calling `Deno.openKv()` directly

**TypeScript errors?**
1. Run `deno task type-check` to see all errors
2. Check generic constraints: `extends BaseGameState`
3. Verify imports use correct paths

## 📝 Message Type Reference

### Core Messages (handled by WebSocket handler)

| Type | Direction | Purpose |
|------|-----------|---------|
| `join-room` | Client → Server | Player joins room |
| `leave-room` | Client → Server | Player leaves room |
| `chat` | Client → Server | Send chat message |
| `player-joined` | Server → Client | Notify player joined |
| `player-left` | Server → Client | Notify player left |
| `game-state-update` | Server → Client | Full state sync |
| `error` | Server → Client | Error occurred |

### Drawing Game Messages

| Type | Direction | Purpose |
|------|-----------|---------|
| `draw` | Client → Server | Drawing command |
| `draw-update-batch` | Server → Client | Batched drawing commands |
| `guess` | Client → Server | Word guess |
| `correct-guess` | Server → Client | Guess was correct |
| `word-selected` | Server → Client | New word chosen |
| `round-end` | Server → Client | Round complete |

## 🎯 Configuration Quick Ref

```typescript
// lib/config.ts
game: {
  maxPlayersPerRoom: 8,        // Room capacity
  roundTimeLimit: 120,         // Seconds per round
  maxRooms: 1000,              // Server-wide limit
  chatMessageLimit: 100,       // Messages per room
}

websocket: {
  maxConnections: 10000,       // Server-wide
  heartbeatInterval: 30000,    // 30 seconds
  connectionTimeout: 60000,    // 1 minute
}

security: {
  rateLimitMessages: 60,       // Per minute
  rateLimitDrawing: 100,       // Per second
  maxMessageLength: 500,       // Characters
}

drawing: {
  serverDebounceMs: 16,        // Command batching delay
  maxBatchSize: 50,            // Force flush threshold
}
```

## 🔐 Environment Variables

```bash
# .env (required for auth only)
JWT_SECRET=your-secret-key-here-minimum-32-chars
DATABASE_URL=postgresql://user:pass@host/db?sslmode=require
DENO_ENV=development  # Optional, auto-detected
```

## 📊 File Size Limits

- Island files: Keep under 300 lines (split if larger)
- Component files: Keep under 200 lines
- Engine files: Keep under 500 lines (extract utils if needed)
- Test files: One file per class/module

## 🎨 Tailwind Class Order

```tsx
// Correct order: layout → spacing → color → other
<div className="flex flex-col items-center gap-4 p-6 bg-blue-500 rounded-lg shadow-lg hover:bg-blue-600 transition-colors">

// Groups:
// 1. Layout: flex, grid, block, inline
// 2. Positioning: relative, absolute, top, left
// 3. Sizing: w-full, h-screen, max-w-lg
// 4. Spacing: p-4, m-2, gap-4
// 5. Typography: text-lg, font-bold
// 6. Colors: bg-*, text-*, border-*
// 7. Effects: shadow, rounded, opacity
// 8. States: hover:, focus:, active:
// 9. Responsive: sm:, md:, lg:
```

## 🧪 Test Patterns

```typescript
// Unit test structure
describe("DrawingGameEngine", () => {
  describe("validateDrawingAction", () => {
    it("should allow current drawer to draw", () => {
      const engine = new DrawingGameEngine();
      const gameState = createMockGameState();
      const result = engine.validateDrawingAction(gameState, "player1", mockCommand);
      assertEquals(result, true);
    });
  });
});

// E2E test structure
test("Player can join room and see lobby", async ({ page }) => {
  await page.goto("http://localhost:3000");
  await page.getByRole("button", { name: "Create Room" }).click();
  await expect(page.getByText("Waiting for players")).toBeVisible();
});
```

## ⚡ Performance Tips

1. **Batch messages**: Don't send 100 WebSocket messages, send 1 batch
2. **Debounce draws**: Use 16ms delay for smooth 60fps rendering
3. **Room-scoped broadcasts**: Only send to players in target room
4. **Cache KV reads**: Store frequently accessed data in memory
5. **Lazy load islands**: Use dynamic imports for heavy components

## 🚨 Common Mistakes to Avoid

1. ❌ Throwing errors in database operations → ✅ Return `DatabaseResult`
2. ❌ Calling `Deno.openKv()` directly → ✅ Use `getAsyncDatabaseService()`
3. ❌ Forgetting `--unstable-kv` flag → ✅ Add to all deno commands
4. ❌ Mutating game state directly → ✅ Return new state object
5. ❌ Using useState in Components → ✅ Move to Island or use props
6. ❌ Missing game registration import → ✅ Add to `lib/games/index.ts`
7. ❌ Leaving console.log in code → ✅ Remove before committing

## 📚 Further Reading

- [ARCHITECTURE.md](./ARCHITECTURE.md) - Deep dive into system design
- [.github/copilot-instructions.md](../.github/copilot-instructions.md) - Comprehensive guide
- [README.md](../README.md) - Project overview and setup
- [Fresh Docs](https://fresh.deno.dev) - Framework documentation
- [Deno KV Guide](https://deno.com/kv) - Database documentation
