# Testing Guide

This directory contains the test suite for the multiplayer drawing game, including both unit tests and end-to-end (E2E) tests using Playwright.

## Test Structure

```
tests/
├── e2e/                          # End-to-end tests with Playwright
│   ├── utils/                    # Test utilities and helpers
│   │   └── test-helpers.ts       # Common test functions
│   ├── smoke.test.ts             # Basic functionality smoke tests
│   ├── game-flow.test.ts         # Complete game flow tests
│   ├── room-management.test.ts   # Room creation/joining tests
│   ├── drawing-functionality.test.ts # Drawing canvas tests
│   ├── websocket-realtime.test.ts # WebSocket and real-time tests
│   └── run-tests.ts              # Test runner script
└── README.md                     # This file
```

## Getting Started

### Prerequisites

1. **Install Playwright browsers:**
   ```bash
   deno task playwright:install
   ```

2. **Start the development server:**
   ```bash
   deno task start
   ```
   The server should be running on `http://localhost:3000`

### Running Tests

#### All E2E Tests
```bash
# Run all tests headless
deno task test:e2e

# Run tests with browser visible (headed mode)
deno task test:e2e:headed

# Run tests in debug mode (step through)
deno task test:e2e:debug
```

#### Specific Test Categories
```bash
# Run only smoke tests
deno task test:e2e --grep "Smoke Tests"

# Run only multiplayer tests
deno task test:e2e --grep "Multiplayer"

# Run only drawing tests
deno task test:e2e --grep "Drawing"
```

#### Browser-Specific Tests
```bash
# Run tests only in Chrome
deno task test:e2e --project chromium

# Run tests only in Firefox
deno task test:e2e --project firefox

# Run tests only in Safari
deno task test:e2e --project webkit
```

#### Unit Tests
```bash
# Run Deno unit tests
deno task test

# Run unit tests in watch mode
deno task test:watch

# Run unit tests with coverage
deno task test:coverage
```

## Test Categories

### 1. Smoke Tests (`smoke.test.ts`)
Basic functionality checks that run quickly:
- Homepage loads successfully
- Navigation works
- No critical JavaScript errors
- Basic responsive design
- API health checks

### 2. Game Flow Tests (`game-flow.test.ts`)
Complete user journey testing:
- Room creation and joining
- Game lobby functionality
- Drawing interactions
- Chat functionality
- Multiplayer synchronization

### 3. Room Management Tests (`room-management.test.ts`)
Room-specific functionality:
- Room creation with valid codes
- Joining existing rooms
- Room not found handling
- Player list management
- Room capacity limits
- Leave room functionality

### 4. Drawing Functionality Tests (`drawing-functionality.test.ts`)
Canvas and drawing features:
- Canvas display and interactions
- Mouse and touch drawing
- Drawing tools (brush, colors, sizes)
- Clear/reset functionality
- Undo/redo operations
- Mobile responsiveness

### 5. WebSocket & Real-time Tests (`websocket-realtime.test.ts`)
Real-time communication testing:
- WebSocket connection establishment
- Connection reconnection handling
- Message synchronization between players
- Drawing synchronization
- Player join/leave events
- Game state synchronization

## Test Utilities

The `test-helpers.ts` file provides utility functions for common test operations:

```typescript
import { GameTestHelpers } from "./utils/test-helpers.ts";

// In your test
const helper = new GameTestHelpers(page);

// Navigate to a room
const roomCode = await helper.navigateToRoom();

// Send a chat message
await helper.sendChatMessage("Hello!");

// Draw on canvas
await helper.drawOnCanvas(50, 50, 100, 100);

// Check WebSocket connection
const hasConnection = await helper.hasWebSocketConnection();
```

## Multiplayer Testing

For testing multiplayer functionality, use the helper functions:

```typescript
import { createMultiplePlayersInRoom, cleanupMultiplePlayers } from "./utils/test-helpers.ts";

test("multiplayer test", async ({ browser }) => {
  const { contexts, pages, roomCode } = await createMultiplePlayersInRoom(browser, 3);
  
  try {
    // Test with multiple players
    const [player1, player2, player3] = pages;
    // ... test logic
  } finally {
    await cleanupMultiplePlayers(contexts);
  }
});
```

## Configuration

### Playwright Configuration (`playwright.config.ts`)
- **Base URL**: `http://localhost:3000`
- **Browsers**: Chrome, Firefox, Safari, Mobile Chrome, Mobile Safari
- **Retries**: 2 retries in CI, 0 locally
- **Screenshots**: On failure only
- **Videos**: Retained on failure
- **Traces**: On first retry

### Test Environment Variables
- `CI=true`: Enables CI-specific settings (more retries, single worker)

## Debugging Tests

### Visual Debugging
```bash
# Run tests with browser visible
deno task test:e2e:headed

# Run specific test in debug mode
deno task test:e2e:debug --grep "specific test name"
```

### Screenshots and Videos
Failed tests automatically capture:
- Screenshots at the point of failure
- Videos of the entire test run
- Traces for detailed debugging

These are saved in the `test-results/` directory.

### Console Logs
WebSocket messages and other debug information are logged during test runs. Check the test output for:
- WebSocket frame sent/received logs
- Console errors and warnings
- Network request logs

## CI/CD Integration

Tests run automatically on:
- Push to `main` or `develop` branches
- Pull requests to `main` or `develop`

The GitHub Actions workflow:
1. Sets up Deno environment
2. Caches dependencies
3. Installs Playwright browsers
4. Runs all E2E tests
5. Uploads test reports as artifacts

## Best Practices

### Writing Tests
1. **Use descriptive test names** that explain what is being tested
2. **Keep tests independent** - each test should work in isolation
3. **Use test helpers** for common operations
4. **Clean up resources** (close contexts, connections)
5. **Handle timing issues** with proper waits and timeouts

### Test Data
1. **Use unique identifiers** for test data to avoid conflicts
2. **Clean up test data** after tests complete
3. **Use realistic test scenarios** that match actual user behavior

### Performance
1. **Run smoke tests first** to catch basic issues quickly
2. **Use parallel execution** where possible
3. **Optimize selectors** for speed and reliability
4. **Minimize wait times** while ensuring stability

## Troubleshooting

### Common Issues

**Tests timing out:**
- Increase timeout values in test configuration
- Check if the development server is running
- Verify WebSocket connections are working

**Element not found:**
- Check if selectors match the actual DOM structure
- Use more flexible selectors that work across different states
- Add proper wait conditions

**WebSocket connection issues:**
- Ensure the server supports WebSocket connections
- Check for network connectivity issues
- Verify WebSocket endpoint URLs

**Flaky tests:**
- Add proper wait conditions instead of fixed timeouts
- Handle race conditions in multiplayer scenarios
- Use retry mechanisms for unreliable operations

### Getting Help

1. Check the test output logs for specific error messages
2. Run tests in headed mode to see what's happening visually
3. Use the Playwright trace viewer for detailed debugging
4. Check the server logs for backend issues

For more information about Playwright, visit: https://playwright.dev/