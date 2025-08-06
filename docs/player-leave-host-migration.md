# Player Leave and Host Migration Implementation

This document describes the implementation of player leave functionality with automatic host migration for the multiplayer drawing game.

## Overview

When a player leaves a game room, the system now:

1. **Removes the player** from the room and database
2. **Transfers host privileges** if the leaving player was the host
3. **Notifies all remaining players** about the player leaving and any host changes
4. **Updates the game state** to reflect the changes
5. **Deletes the room** if no players remain

## Key Features

### Host Migration
- When the host leaves, the system automatically promotes the next player (first remaining player) to host
- Host transfer is atomic - either succeeds completely or fails safely
- All remaining players are notified of the host change

### Real-time Updates
- All players receive immediate WebSocket notifications when someone leaves
- Game state is updated to remove the leaving player from scores and drawing rotation
- If the leaving player was currently drawing, the game resets to waiting state

### Room Cleanup
- Rooms with no remaining players are automatically deleted
- Database cleanup prevents orphaned data

## Implementation Details

### Room Manager Updates

The `RoomManager.leaveRoom()` method now returns detailed information:

```typescript
interface LeaveRoomResult {
  success: boolean;
  data?: {
    wasHost: boolean;           // Whether the leaving player was host
    newHostId?: string;         // ID of the new host (if host migration occurred)
    newHostName?: string;       // Name of the new host
    roomDeleted?: boolean;      // Whether the room was deleted
    remainingPlayers?: Player[]; // List of remaining players
  };
  error?: string;
}
```

### WebSocket Message Types

New message types for host migration:

```typescript
// Room update with host migration info
{
  type: "room-update",
  roomId: string,
  data: {
    type: "player-left",
    playerId: string,
    playerName: string,
    wasHost: boolean,
    hostMigration?: {
      newHostId: string,
      newHostName: string
    }
  }
}

// Dedicated host change notification
{
  type: "room-update",
  roomId: string,
  data: {
    type: "host-changed",
    oldHostId: string,
    oldHostName: string,
    newHostId: string,
    newHostName: string
  }
}
```

### Database Updates

Enhanced the `updateRoom` method to properly handle camelCase to snake_case field mapping:

```typescript
updateRoom(id: string, updates: Partial<Room>): DatabaseResult<boolean> {
  // Maps camelCase fields to database snake_case fields
  const fieldMapping = {
    hostId: 'host_id',
    maxPlayers: 'max_players',
    isActive: 'is_active',
    // ...
  };
}
```

## Usage Examples

### Basic Player Leave
```typescript
const leaveResult = await roomManager.leaveRoom(roomId, playerId);

if (leaveResult.success) {
  const { wasHost, newHostId, roomDeleted } = leaveResult.data;
  
  if (roomDeleted) {
    console.log("Room was deleted - no players remaining");
  } else if (wasHost && newHostId) {
    console.log(`Host migrated to player ${newHostId}`);
  }
}
```

### WebSocket Handler Integration
```typescript
// In room handler
await this.removePlayerFromRoom(playerId, roomId);

// This automatically:
// 1. Removes player from database
// 2. Transfers host if needed
// 3. Broadcasts updates to all remaining players
// 4. Updates game state
```

## Error Handling

The implementation includes comprehensive error handling:

- **Database failures**: Atomic operations with rollback on failure
- **Host migration failures**: Prevents room corruption
- **WebSocket failures**: Graceful degradation with fallback notifications
- **Race conditions**: Proper sequencing of database operations

## Testing

Comprehensive unit tests cover:

- ✅ Non-host player leaving (no host migration)
- ✅ Host player leaving (triggers host migration)
- ✅ Last player leaving (room deletion)
- ✅ Edge cases (non-existent rooms/players)
- ✅ Error scenarios and rollback behavior

## Performance Considerations

- **Minimal database queries**: Single transaction for player removal and host transfer
- **Efficient broadcasting**: Only sends updates to affected room members
- **Memory cleanup**: Removes player data from all relevant data structures
- **Connection pooling**: Reuses database connections for better performance

## Security

- **Validation**: All player and room IDs are validated before operations
- **Authorization**: Only valid room members can leave rooms
- **Data integrity**: Foreign key constraints prevent orphaned data
- **Rate limiting**: WebSocket message broadcasting respects rate limits

## Future Enhancements

Potential improvements for future versions:

1. **Host selection criteria**: Allow custom logic for choosing new host
2. **Graceful reconnection**: Handle temporary disconnections vs. intentional leaves
3. **Host privileges**: More granular host permissions and delegation
4. **Analytics**: Track host migration patterns and room lifecycle metrics