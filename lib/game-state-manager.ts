import type { D1Database, KVNamespace } from "../types/cloudflare.ts";
import type { GameState, PlayerState, GameSession, Score } from "../types/game.ts";
import { KVStorageService } from "./kv-storage.ts";
import { GameSessionDAO } from "./dao/game-session-dao.ts";
import { PlayerDAO } from "./dao/player-dao.ts";
import { RoomDAO } from "./dao/room-dao.ts";

export interface GameStateManagerOptions {
  roundDurationMs: number;
  maxRounds: number;
  pointsForCorrectGuess: number;
  timeBasedScoringEnabled: boolean;
}

export class GameStateManager {
  private kvService: KVStorageService;
  private gameSessionDAO: GameSessionDAO;
  private playerDAO: PlayerDAO;
  private roomDAO: RoomDAO;
  private options: GameStateManagerOptions;

  constructor(
    db: D1Database,
    kv: KVNamespace,
    options: Partial<GameStateManagerOptions> = {}
  ) {
    this.kvService = new KVStorageService(kv);
    this.gameSessionDAO = new GameSessionDAO(db);
    this.playerDAO = new PlayerDAO(db);
    this.roomDAO = new RoomDAO(db);
    
    this.options = {
      roundDurationMs: 120000, // 2 minutes
      maxRounds: 5,
      pointsForCorrectGuess: 100,
      timeBasedScoringEnabled: true,
      ...options
    };
  }

  /**
   * Initialize a new game state for a room
   */
  async initializeGameState(roomId: string, hostId: string): Promise<GameState> {
    const playersResult = await this.playerDAO.findByRoom(roomId);
    if (!playersResult.success || !playersResult.data) {
      throw new Error('Failed to get players for room');
    }

    const players: PlayerState[] = playersResult.data.map(player => ({
      id: player.id,
      name: player.name,
      isHost: player.isHost,
      isConnected: true,
      lastActivity: Date.now()
    }));

    const gameState: GameState = {
      roomId,
      currentDrawer: '',
      currentWord: '',
      roundNumber: 0,
      timeRemaining: this.options.roundDurationMs,
      phase: 'waiting',
      players,
      scores: players.reduce((acc, player) => {
        acc[player.id] = 0;
        return acc;
      }, {} as Record<string, number>),
      drawingData: []
    };

    await this.kvService.setGameState(roomId, gameState);
    return gameState;
  }

  /**
   * Get current game state for a room
   */
  async getGameState(roomId: string): Promise<GameState | null> {
    return await this.kvService.getGameState(roomId);
  }

  /**
   * Start a new game session
   */
  async startGame(roomId: string): Promise<GameState> {
    let gameState = await this.getGameState(roomId);
    if (!gameState) {
      throw new Error('Game state not found');
    }

    // Create a new game session in the database
    const sessionResult = await this.gameSessionDAO.create({
      roomId,
      totalRounds: this.options.maxRounds
    });

    if (!sessionResult.success) {
      throw new Error('Failed to create game session');
    }

    // Start the first round
    gameState = await this.startNewRound(gameState);
    return gameState;
  }

  /**
   * Start a new round with turn rotation
   */
  async startNewRound(gameState: GameState): Promise<GameState> {
    const activePlayers = gameState.players.filter(p => p.isConnected);
    if (activePlayers.length < 2) {
      throw new Error('Not enough players to start a round');
    }

    // Rotate to next drawer
    const nextDrawer = this.getNextDrawer(gameState);
    
    const updatedState: GameState = {
      ...gameState,
      currentDrawer: nextDrawer.id,
      currentWord: this.generateRandomWord(),
      roundNumber: gameState.roundNumber + 1,
      timeRemaining: this.options.roundDurationMs,
      phase: 'drawing',
      drawingData: [] // Clear previous drawing
    };

    await this.kvService.setGameState(gameState.roomId, updatedState);
    return updatedState;
  }

  /**
   * Process a correct guess and calculate score
   */
  async processCorrectGuess(
    roomId: string, 
    playerId: string, 
    guessTime: number
  ): Promise<GameState> {
    const gameState = await this.getGameState(roomId);
    if (!gameState) {
      throw new Error('Game state not found');
    }

    if (gameState.phase !== 'drawing') {
      throw new Error('Cannot guess during this phase');
    }

    if (playerId === gameState.currentDrawer) {
      throw new Error('Drawer cannot guess');
    }

    // Calculate time-based score
    const points = this.calculateScore(guessTime, gameState.timeRemaining);
    
    // Update scores
    const updatedScores = {
      ...gameState.scores,
      [playerId]: gameState.scores[playerId] + points
    };

    // Award points to drawer as well
    const drawerPoints = Math.floor(points * 0.5); // 50% of guesser points
    updatedScores[gameState.currentDrawer] = gameState.scores[gameState.currentDrawer] + drawerPoints;

    const updatedState: GameState = {
      ...gameState,
      scores: updatedScores,
      phase: 'results'
    };

    await this.kvService.setGameState(roomId, updatedState);
    return updatedState;
  }

  /**
   * End current round and transition to next phase
   */
  async endRound(roomId: string): Promise<GameState> {
    const gameState = await this.getGameState(roomId);
    if (!gameState) {
      throw new Error('Game state not found');
    }

    let updatedState: GameState;

    if (gameState.roundNumber >= this.options.maxRounds) {
      // Game is complete
      updatedState = await this.endGame(gameState);
    } else {
      // Prepare for next round
      updatedState = {
        ...gameState,
        phase: 'waiting',
        timeRemaining: this.options.roundDurationMs
      };
    }

    await this.kvService.setGameState(roomId, updatedState);
    return updatedState;
  }

  /**
   * End the game and determine winner
   */
  async endGame(gameState: GameState): Promise<GameState> {
    const winner = this.determineWinner(gameState);
    
    // Update game session in database
    const currentSessionResult = await this.gameSessionDAO.getCurrentSession(gameState.roomId);
    if (currentSessionResult.success && currentSessionResult.data) {
      await this.gameSessionDAO.end(currentSessionResult.data.id, winner?.id);
    }

    const updatedState: GameState = {
      ...gameState,
      phase: 'results',
      currentDrawer: '',
      currentWord: ''
    };

    return updatedState;
  }

  /**
   * Update player connection status
   */
  async updatePlayerConnection(
    roomId: string, 
    playerId: string, 
    isConnected: boolean
  ): Promise<GameState> {
    const gameState = await this.getGameState(roomId);
    if (!gameState) {
      throw new Error('Game state not found');
    }

    const updatedPlayers = gameState.players.map(player => 
      player.id === playerId 
        ? { ...player, isConnected, lastActivity: Date.now() }
        : player
    );

    const updatedState: GameState = {
      ...gameState,
      players: updatedPlayers
    };

    await this.kvService.setGameState(roomId, updatedState);
    return updatedState;
  }

  /**
   * Add a new player to the game state
   */
  async addPlayer(roomId: string, playerId: string, playerName: string): Promise<GameState> {
    const gameState = await this.getGameState(roomId);
    if (!gameState) {
      throw new Error('Game state not found');
    }

    // Check if player already exists
    const existingPlayer = gameState.players.find(p => p.id === playerId);
    if (existingPlayer) {
      return await this.updatePlayerConnection(roomId, playerId, true);
    }

    const newPlayer: PlayerState = {
      id: playerId,
      name: playerName,
      isHost: false,
      isConnected: true,
      lastActivity: Date.now()
    };

    const updatedState: GameState = {
      ...gameState,
      players: [...gameState.players, newPlayer],
      scores: {
        ...gameState.scores,
        [playerId]: 0
      }
    };

    await this.kvService.setGameState(roomId, updatedState);
    return updatedState;
  }

  /**
   * Remove a player from the game state
   */
  async removePlayer(roomId: string, playerId: string): Promise<GameState> {
    const gameState = await this.getGameState(roomId);
    if (!gameState) {
      throw new Error('Game state not found');
    }

    const updatedPlayers = gameState.players.filter(p => p.id !== playerId);
    const updatedScores = { ...gameState.scores };
    delete updatedScores[playerId];

    let updatedState: GameState = {
      ...gameState,
      players: updatedPlayers,
      scores: updatedScores
    };

    // If the current drawer left, end the round
    if (gameState.currentDrawer === playerId && gameState.phase === 'drawing') {
      updatedState = await this.endRound(roomId);
    }

    await this.kvService.setGameState(roomId, updatedState);
    return updatedState;
  }

  /**
   * Get next drawer in rotation
   */
  private getNextDrawer(gameState: GameState): PlayerState {
    const activePlayers = gameState.players.filter(p => p.isConnected);
    
    if (activePlayers.length === 0) {
      throw new Error('No active players');
    }

    // If no current drawer, start with first player
    if (!gameState.currentDrawer) {
      return activePlayers[0];
    }

    // Find current drawer index
    const currentIndex = activePlayers.findIndex(p => p.id === gameState.currentDrawer);
    
    // Get next player in rotation (wrap around if needed)
    const nextIndex = (currentIndex + 1) % activePlayers.length;
    return activePlayers[nextIndex];
  }

  /**
   * Calculate score based on time remaining
   */
  private calculateScore(guessTime: number, timeRemaining: number): number {
    if (!this.options.timeBasedScoringEnabled) {
      return this.options.pointsForCorrectGuess;
    }

    const totalTime = this.options.roundDurationMs;
    const timeUsed = totalTime - timeRemaining;
    const timeRatio = Math.max(0, (totalTime - timeUsed) / totalTime);
    
    // Score ranges from 50% to 100% of base points based on speed
    const minScore = Math.floor(this.options.pointsForCorrectGuess * 0.5);
    const maxScore = this.options.pointsForCorrectGuess;
    
    return Math.floor(minScore + (maxScore - minScore) * timeRatio);
  }

  /**
   * Determine the winner based on scores
   */
  private determineWinner(gameState: GameState): PlayerState | null {
    const activePlayers = gameState.players.filter(p => p.isConnected);
    if (activePlayers.length === 0) return null;

    let winner = activePlayers[0];
    let highestScore = gameState.scores[winner.id] || 0;

    for (const player of activePlayers) {
      const score = gameState.scores[player.id] || 0;
      if (score > highestScore) {
        highestScore = score;
        winner = player;
      }
    }

    return winner;
  }

  /**
   * Generate a random word for drawing
   */
  private generateRandomWord(): string {
    const words = [
      'cat', 'dog', 'house', 'tree', 'car', 'sun', 'moon', 'star',
      'flower', 'bird', 'fish', 'book', 'chair', 'table', 'phone',
      'computer', 'pizza', 'cake', 'apple', 'banana', 'guitar',
      'piano', 'mountain', 'ocean', 'rainbow', 'butterfly', 'elephant',
      'lion', 'tiger', 'bear', 'rabbit', 'horse', 'cow', 'sheep',
      'chicken', 'duck', 'frog', 'snake', 'spider', 'ant', 'bee'
    ];
    
    return words[Math.floor(Math.random() * words.length)];
  }

  /**
   * Clean up expired game states
   */
  async cleanup(roomId: string): Promise<void> {
    await this.kvService.deleteGameState(roomId);
    await this.kvService.clearDrawingData(roomId);
  }
}