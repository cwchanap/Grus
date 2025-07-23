/**
 * Example usage of GameStateManager
 * This demonstrates the typical flow of a multiplayer drawing game
 */

import { GameStateManager } from "./game-state-manager.ts";
import type { GameState } from "../types/game.ts";

// Mock implementations for example
class MockKV {
  private storage = new Map<string, string>();
  
  async get(key: string, options?: { type?: string }) {
    const value = this.storage.get(key);
    if (!value) return null;
    return options?.type === "json" ? JSON.parse(value) : value;
  }
  
  async put(key: string, value: string) {
    this.storage.set(key, value);
  }
  
  async delete(key: string) {
    this.storage.delete(key);
  }
  
  async list() {
    return { keys: [] };
  }
}

class MockDB {
  async prepare() {
    return {
      bind: () => ({
        first: async () => null,
        all: async () => ({ results: [] }),
        run: async () => ({ success: true, meta: { changes: 1 } })
      })
    };
  }
}

export async function demonstrateGameFlow() {
  console.log("🎮 Multiplayer Drawing Game - State Management Demo");
  console.log("=" .repeat(50));

  const mockDB = new MockDB();
  const mockKV = new MockKV();
  
  const gameManager = new GameStateManager(
    mockDB as any,
    mockKV as any,
    {
      roundDurationMs: 120000, // 2 minutes
      maxRounds: 3,
      pointsForCorrectGuess: 100,
      timeBasedScoringEnabled: true
    }
  );

  const roomId = "demo-room-123";

  try {
    // Step 1: Create initial game state manually (bypassing database)
    console.log("\n1. Creating initial game state...");
    const initialState: GameState = {
      roomId,
      currentDrawer: '',
      currentWord: '',
      roundNumber: 0,
      timeRemaining: 120000,
      phase: 'waiting',
      players: [],
      scores: {},
      drawingData: []
    };
    
    // Store it in KV
    await mockKV.put(`game:${roomId}`, JSON.stringify(initialState));
    console.log(`   Room: ${initialState.roomId}`);
    console.log(`   Phase: ${initialState.phase}`);
    console.log(`   Players: ${initialState.players.length}`);

    // Step 2: Add players to the game
    console.log("\n2. Adding players...");
    let gameState = await gameManager.addPlayer(roomId, "player-1", "Alice");
    gameState = await gameManager.addPlayer(roomId, "player-2", "Bob");
    gameState = await gameManager.addPlayer(roomId, "player-3", "Charlie");
    
    console.log(`   Total players: ${gameState.players.length}`);
    gameState.players.forEach(player => {
      console.log(`   - ${player.name} (${player.id}) ${player.isHost ? '[HOST]' : ''}`);
    });

    // Step 3: Start the first round manually (bypassing database session creation)
    console.log("\n3. Starting first round...");
    gameState = await gameManager.startNewRound(gameState);
    console.log(`   Phase: ${gameState.phase}`);
    console.log(`   Round: ${gameState.roundNumber}`);
    console.log(`   Current drawer: ${gameState.players.find(p => p.id === gameState.currentDrawer)?.name}`);
    console.log(`   Word to draw: ${gameState.currentWord}`);

    // Step 4: Simulate correct guesses
    console.log("\n4. Simulating gameplay...");
    
    // Player 2 guesses correctly after 30 seconds
    const guessTime1 = Date.now();
    gameState = await gameManager.processCorrectGuess(roomId, "player-2", guessTime1);
    console.log(`   Player 2 guessed correctly! Score: ${gameState.scores["player-2"]}`);
    console.log(`   Drawer bonus: ${gameState.scores[gameState.currentDrawer]}`);

    // End current round
    gameState = await gameManager.endRound(roomId);
    console.log(`   Round ended. Phase: ${gameState.phase}`);

    // Step 5: Start next round
    console.log("\n5. Starting next round...");
    gameState = await gameManager.startNewRound(gameState);
    console.log(`   Round: ${gameState.roundNumber}`);
    console.log(`   New drawer: ${gameState.players.find(p => p.id === gameState.currentDrawer)?.name}`);
    console.log(`   New word: ${gameState.currentWord}`);

    // Step 6: Simulate more gameplay
    console.log("\n6. More gameplay simulation...");
    
    // Player 3 guesses correctly after 45 seconds
    const guessTime2 = Date.now();
    gameState = await gameManager.processCorrectGuess(roomId, "player-3", guessTime2);
    console.log(`   Player 3 guessed correctly! Score: ${gameState.scores["player-3"]}`);

    // End round
    gameState = await gameManager.endRound(roomId);

    // Step 7: Show final scores
    console.log("\n7. Current Scores:");
    Object.entries(gameState.scores).forEach(([playerId, score]) => {
      const player = gameState.players.find(p => p.id === playerId);
      console.log(`   ${player?.name}: ${score} points`);
    });

    // Step 8: Simulate player disconnection
    console.log("\n8. Simulating player disconnection...");
    gameState = await gameManager.updatePlayerConnection(roomId, "player-3", false);
    const connectedPlayers = gameState.players.filter(p => p.isConnected);
    console.log(`   Connected players: ${connectedPlayers.length}`);
    connectedPlayers.forEach(player => {
      console.log(`   - ${player.name} (connected)`);
    });

    // Step 9: Continue with remaining players
    console.log("\n9. Continuing with remaining players...");
    if (gameState.roundNumber < 3) {
      gameState = await gameManager.startNewRound(gameState);
      console.log(`   Round: ${gameState.roundNumber}`);
      console.log(`   Drawer: ${gameState.players.find(p => p.id === gameState.currentDrawer)?.name}`);
    }

    // Step 10: End game and determine winner
    console.log("\n10. Game completion simulation...");
    // Simulate completing all rounds by manually setting round number
    const finalGameState = { ...gameState, roundNumber: 3 };
    const endedGameState = await gameManager.endGame(finalGameState);
    
    console.log(`   Game ended! Phase: ${endedGameState.phase}`);
    
    // Determine winner
    const finalConnectedPlayers = endedGameState.players.filter(p => p.isConnected);
    if (finalConnectedPlayers.length > 0) {
      const winner = finalConnectedPlayers.reduce((prev, current) => 
        (endedGameState.scores[current.id] || 0) > (endedGameState.scores[prev.id] || 0) ? current : prev
      );
      
      console.log(`   🏆 Winner: ${winner.name} with ${endedGameState.scores[winner.id]} points!`);
    } else {
      console.log(`   No connected players to determine winner`);
    }

    // Step 11: Cleanup
    console.log("\n11. Cleaning up...");
    await gameManager.cleanup(roomId);
    console.log("   Game state cleaned up successfully");

    console.log("\n✅ Demo completed successfully!");

  } catch (error) {
    console.error("❌ Demo failed:", error.message);
  }
}

// Run the demo if this file is executed directly
if (import.meta.main) {
  await demonstrateGameFlow();
}