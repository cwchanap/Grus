---
inclusion: always
---

# Product Rules & Implementation Guidelines

Real-time multiplayer drawing and guessing game. Follow these constraints and patterns when implementing any feature.

## Critical Business Rules

### Room Management (MUST Enforce)
- **Player limit**: NEVER allow more than 8 players per room - validate on join/create
- **Room codes**: MUST be exactly 6 characters, alphanumeric uppercase (A-Z, 0-9)
- **Room cleanup**: Auto-delete rooms after 30 minutes of inactivity
- **Host assignment**: Room creator is host; auto-promote on host disconnect
- **Validation pattern**: `^[A-Z0-9]{6}$` for room codes

### Game State Management
- **Turn system**: Only one player draws at a time, others guess simultaneously
- **Round timing**: 60-90 seconds per turn (host configurable, default 75s)
- **Scoring algorithm**: `Math.max(0, 100 - Math.floor(timeElapsed / totalTime * 100))`
- **Word selection**: Random from difficulty-appropriate word list
- **Drawing tools**: 3 brush sizes (2px, 5px, 10px), 8 colors, eraser only

### Chat & Security
- **Answer filtering**: MUST hide messages containing current word (case-insensitive)
- **Rate limiting**: Max 5 messages per 10 seconds per player
- **Content filtering**: Block profanity in chat messages and room names
- **Message validation**: Max 100 characters, trim whitespace

## Implementation Patterns

### Real-time Communication
- **WebSocket priority**: All game state changes via WebSocket first
- **Latency targets**: Drawing strokes <50ms, chat messages <100ms
- **State sync**: Broadcast state changes to all room members immediately
- **Connection recovery**: Auto-reconnect with exponential backoff (1s, 2s, 4s, 8s max)

### Data Validation
```typescript
// Room validation
const isValidRoomCode = (code: string) => /^[A-Z0-9]{6}$/.test(code);
const isValidPlayerCount = (count: number) => count >= 1 && count <= 8;

// Drawing data validation  
const isValidStroke = (stroke: any) => 
  stroke.points?.length > 0 && stroke.points.length <= 1000;
```

### Error Handling Requirements
- **Connection loss**: Show offline indicator, queue actions for replay
- **Invalid state**: Reset to last known good state, notify users
- **Host migration**: Seamlessly transfer host privileges to next player
- **Drawing conflicts**: Server-side stroke ordering is authoritative

## Performance Constraints

### Resource Limits
- **Drawing data**: Max 10KB per complete drawing
- **Memory usage**: Limit canvas operations to prevent mobile crashes
- **Database**: Batch operations, use prepared statements
- **WebSocket**: Max 100 messages/second per connection

### Mobile Optimization
- **Touch targets**: Minimum 44px for all interactive elements
- **Canvas scaling**: Maintain 16:9 aspect ratio, scale to fit screen
- **Gesture handling**: Disable zoom on drawing area, enable tool selection
- **Keyboard**: Auto-hide virtual keyboard when drawing mode active

## Architecture Decisions

### State Management
- **Client state**: Optimistic updates for drawing, authoritative for game state
- **Server state**: Single source of truth stored in Cloudflare KV
- **Persistence**: Game sessions in KV, player stats in D1 database
- **Caching**: Aggressive caching for static assets, no cache for game state

### Security Requirements
- **Input sanitization**: Escape all user content before display
- **Rate limiting**: Implement at WebSocket and HTTP levels
- **CORS**: Restrict to known domains in production
- **Content validation**: Server-side validation for all game actions

### Testing Requirements
- **Real-time testing**: All features must work with simulated network delays
- **Mobile testing**: Test on actual devices, not just browser dev tools
- **Load testing**: Verify performance with 8 concurrent players per room
- **Error scenarios**: Test all connection failure and recovery paths