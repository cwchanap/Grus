# WebSocket Module Refactoring

This directory contains the refactored WebSocket implementation for the drawing and guessing game. The code has been restructured from a monolithic approach into a modular, maintainable architecture.

## Architecture Overview

### Core Components (`core/`)

- **`websocket-server.ts`** - Main WebSocket server that orchestrates all components
- **`connection-pool.ts`** - Manages WebSocket connections and room mappings
- **`message-router.ts`** - Routes and validates incoming messages

### Message Handlers (`handlers/`)

- **`room-handler.ts`** - Handles room join/leave operations
- **`chat-handler.ts`** - Processes chat messages and correct guesses
- **`drawing-handler.ts`** - Manages drawing operations
- **`game-handler.ts`** - Controls game state and round management

### Services (`services/`)

- **`game-state-service.ts`** - Game state persistence (KV storage)
- **`player-service.ts`** - Player and room database operations
- **`timer-service.ts`** - Round timer management

### Utilities (`utils/`)

- **`message-validator.ts`** - Message validation logic
- **`rate-limiter.ts`** - Rate limiting implementation
- **`word-generator.ts`** - Random word selection

### Types (`types/`)

- **`websocket-internal.ts`** - Internal type definitions

## Key Benefits

1. **Single Responsibility** - Each class has one focused purpose
2. **Testability** - Components can be unit tested in isolation
3. **Maintainability** - Smaller files are easier to understand and modify
4. **Extensibility** - New features can be added without touching existing code
5. **Dependency Injection** - Better separation of concerns and testability

## Usage

### New Code (Recommended)

```typescript
import { WebSocketServer } from "./lib/websocket/core/websocket-server.ts";

const server = new WebSocketServer(env);
const response = server.handleWebSocketUpgrade(request);
```

### Legacy Code (Backward Compatible)

```typescript
import { WebSocketHandler } from "./lib/websocket/websocket-handler.ts";

const handler = new WebSocketHandler(env);
const response = handler.handleWebSocketUpgrade(request);
```

### WebSocket Manager (Unchanged)

```typescript
import { WebSocketManager } from "./lib/websocket/websocket-manager.ts";

const manager = new WebSocketManager(env);
const response = await manager.handleRequest(request);
```

## Environment Support

The refactored code maintains support for both:

- **Cloudflare Workers** (production environment)
- **Deno** (development environment)

## Migration Path

1. **Phase 1** ✅ - Refactor into modular components
2. **Phase 2** - Update existing code to use new imports
3. **Phase 3** - Remove legacy compatibility layer
4. **Phase 4** - Add comprehensive unit tests

## Testing Strategy

With the new modular structure, testing becomes much easier:

```typescript
// Example: Testing the chat handler in isolation
import { ChatHandler } from "./handlers/chat-handler.ts";

const mockConnectionPool = new MockConnectionPool();
const mockValidator = new MockMessageValidator();
const mockGameStateService = new MockGameStateService();

const chatHandler = new ChatHandler(
  mockConnectionPool,
  mockValidator,
  mockGameStateService,
);

// Test specific functionality
await chatHandler.handle(mockConnection, mockMessage);
```

## Performance Considerations

- **Connection Pooling** - Efficient WebSocket connection management
- **Message Routing** - Fast message dispatch to appropriate handlers
- **Rate Limiting** - Per-player rate limiting to prevent abuse
- **Memory Management** - Proper cleanup of resources and timers

## Security Features

- **Input Validation** - All messages are validated before processing
- **Rate Limiting** - Prevents message flooding
- **Connection Management** - Proper handling of connection failures
- **Error Handling** - Graceful error recovery

## Future Enhancements

The modular structure enables:

- Plugin-based message handlers
- Multiple storage backends
- Advanced monitoring and metrics
- A/B testing of game features
- Real-time analytics
