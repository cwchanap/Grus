/**
 * Example demonstrating the game flow and turn management functionality
 * This shows how the different components work together to manage a multiplayer drawing game
 */

import { GameStateManager } from "./game-state-manager.ts";
import { WebSocketHandler } from "./websocket/websocket-handler.ts";
import type { GameState, ClientMessage } from "../types/game.ts";

// This example demonstrates the complete game flow:
// 1. Game initialization
// 2. Starting a game with turn rotation
// 3. Processing guesses and scoring
// 4. Round transitions
// 5. Game completion

export class GameFlowExample {
  private gameStateManager: GameStateManager;
  private wsHandler: WebSocketHandler;

  constructor(env: any) {
    this.gameStateManager = new GameStateManager(env.DB, env.GAME_STATE);
    this.wsHandler = new WebSocketHandler(env);
  }

  /**
   * Example of complete game flow from start to finish
   */
  async demonstrateGameFlow() {
    const roomId = "demo-room";
    const hostId = "host-player";

    console.log("=== Game Flow Demonstration ===\n");

    // Step 1: Initialize game state
    console.log("1. Initializing game state...");
    const initialState = await this.gameStateManager.initializeGameState(roomId, hostId);
    console.log(`   - Room: ${initialState.roomId}`);
    console.log(`   - Players: ${initialState.players.length}`);
    console.log(`   - Phase: ${initialState.phase}`);
    console.log(`   - Round: ${initialState.roundNumber}\n`);

    // Step 2: Start the game
    console.log("2. Starting the game...");
    const gameStarted = await this.gameStateManager.startGame(roomId);
    console.log(`   - Current drawer: ${gameStarted.currentDrawer}`);
    console.log(`   - Word to draw: ${gameStarted.currentWord}`);
    console.log(`   - Phase: ${gameStarted.phase}`);
    console.log(`   - Round: ${gameStarted.roundNumber}`);
    console.log(`   - Time remaining: ${gameStarted.timeRemaining}ms\n`);

    // Step 3: Simulate a correct guess
    console.log("3. Processing a correct guess...");
    const guesserPlayerId = gameStarted.players.find(p => p.id !== gameStarted.currentDrawer)?.id;
    if (guesserPlayerId) {
      const afterGuess = await this.gameStateManager.processCorrectGuess(
        roomId, 
        guesserPlayerId, 
        Date.now()
      );
      console.log(`   - Guesser (${guesserPlayerId}) score: ${afterGuess.scores[guesserPlayerId]}`);
      console.log(`   - Drawer (${gameStarted.currentDrawer}) score: ${afterGuess.scores[gameStarted.currentDrawer]}`);
      console.log(`   - Phase: ${afterGuess.phase}\n`);
    }

    // Step 4: End current round and start next
    console.log("4. Ending round and starting next...");
    const roundEnded = await this.gameStateManager.endRound(roomId);
    console.log(`   - Phase after round end: ${roundEnded.phase}`);
    
    if (roundEnded.phase === 'waiting') {
      const nextRound = await this.gameStateManager.startNewRound(roundEnded);
      console.log(`   - New drawer: ${nextRound.currentDrawer}`);
      console.log(`   - New word: ${nextRound.currentWord}`);
      console.log(`   - Round number: ${nextRound.roundNumber}\n`);
    }

    // Step 5: Demonstrate turn rotation
    console.log("5. Demonstrating turn rotation...");
    let currentState = await this.gameStateManager.getGameState(roomId);
    if (currentState) {
      for (let i = 0; i < 3; i++) {
        const previousDrawer = currentState.currentDrawer;
        currentState = await this.gameStateManager.startNewRound(currentState);
        console.log(`   Round ${currentState.roundNumber}: ${previousDrawer} → ${currentState.currentDrawer}`);
      }
    }

    console.log("\n=== Game Flow Complete ===");
  }

  /**
   * Example of WebSocket message handling for game flow
   */
  async demonstrateWebSocketFlow() {
    console.log("\n=== WebSocket Game Flow Messages ===\n");

    // Example messages that would be sent during game flow
    const exampleMessages: ClientMessage[] = [
      {
        type: "start-game",
        roomId: "demo-room",
        playerId: "host-player",
        data: {}
      },
      {
        type: "chat",
        roomId: "demo-room", 
        playerId: "player2",
        data: { text: "cat" } // Correct guess
      },
      {
        type: "next-round",
        roomId: "demo-room",
        playerId: "host-player", 
        data: {}
      },
      {
        type: "end-game",
        roomId: "demo-room",
        playerId: "host-player",
        data: {}
      }
    ];

    console.log("Example WebSocket messages for game flow:");
    exampleMessages.forEach((msg, index) => {
      console.log(`${index + 1}. ${msg.type.toUpperCase()}`);
      console.log(`   Room: ${msg.roomId}`);
      console.log(`   Player: ${msg.playerId}`);
      console.log(`   Data:`, msg.data);
      console.log("");
    });

    console.log("These messages would trigger:");
    console.log("- Game state transitions");
    console.log("- Turn rotation");
    console.log("- Score calculations");
    console.log("- Real-time broadcasts to all players");
    console.log("- Timer management");
  }

  /**
   * Example of game phases and transitions
   */
  demonstrateGamePhases() {
    console.log("\n=== Game Phase Transitions ===\n");

    const phases = [
      {
        phase: "waiting",
        description: "Players are in lobby, waiting for game to start",
        actions: ["Host can start game", "Players can join/leave"]
      },
      {
        phase: "drawing", 
        description: "Current drawer is drawing, others are guessing",
        actions: ["Drawer draws", "Others send guesses", "Timer counts down"]
      },
      {
        phase: "results",
        description: "Round ended, showing scores and results", 
        actions: ["Display scores", "Show correct answer", "Prepare next round"]
      }
    ];

    phases.forEach((phaseInfo, index) => {
      console.log(`${index + 1}. ${phaseInfo.phase.toUpperCase()} Phase`);
      console.log(`   ${phaseInfo.description}`);
      console.log(`   Available actions:`);
      phaseInfo.actions.forEach(action => {
        console.log(`   - ${action}`);
      });
      console.log("");
    });

    console.log("Phase transitions:");
    console.log("waiting → drawing (game/round start)");
    console.log("drawing → results (correct guess or timeout)");
    console.log("results → drawing (next round) or results (game end)");
  }
}

// Example usage (would be called from a route or test)
export async function runGameFlowDemo(env: any) {
  const demo = new GameFlowExample(env);
  
  await demo.demonstrateGameFlow();
  await demo.demonstrateWebSocketFlow();
  demo.demonstrateGamePhases();
}