import { BaseGameEngine } from "../../core/game-engine.ts";
import {
  BettingRound,
  PokerAction,
  PokerClientMessage,
  PokerGameSettings,
  PokerGameState,
  PokerPlayer,
  PokerServerMessage,
} from "../../../types/games/poker.ts";
import { PlayerState } from "../../../types/core/room.ts";
import { compareHands, createDeck, dealCards, evaluateHand, shuffleDeck } from "./poker-utils.ts";

export class PokerGameEngine extends BaseGameEngine<
  PokerGameState,
  PokerGameSettings,
  PokerClientMessage,
  PokerServerMessage
> {
  getGameType(): string {
    return "poker";
  }

  initializeGame(
    roomId: string,
    players: PlayerState[],
    settings: PokerGameSettings,
  ): PokerGameState {
    const pokerPlayers: PokerPlayer[] = players.map((p, i) => ({
      ...p,
      chips: settings.buyIn,
      cards: [],
      bet: 0,
      hasActed: false,
      isAllIn: false,
      isFolded: false,
      position: i,
    }));

    return {
      roomId,
      gameType: "poker",
      phase: "waiting",
      players: pokerPlayers,
      deck: [],
      communityCards: [],
      pot: 0,
      currentBet: 0,
      bettingRound: BettingRound.PRE_FLOP,
      currentPlayerIndex: 0,
      smallBlindIndex: 0,
      bigBlindIndex: 1,
      settings,
      roundNumber: 0,
      timeRemaining: 0,
      chatMessages: [],
      scores: {},
      gameData: {},
    };
  }

  override startGame(gameState: PokerGameState): PokerGameState {
    return this.dealNewHand(gameState);
  }

  handleClientMessage(
    gameState: PokerGameState,
    message: PokerClientMessage,
  ): {
    updatedState: PokerGameState;
    serverMessages: PokerServerMessage[];
  } {
    if (!this.validateGameAction(gameState, message.playerId, message.data)) {
      // Optionally, send an error message to the player
      return { updatedState: gameState, serverMessages: [] };
    }

    let updatedState = { ...gameState };
    const playerIndex = updatedState.players.findIndex(
      (p) => p.id === message.playerId,
    );
    const player = updatedState.players[playerIndex];

    switch (message.data.action) {
      case PokerAction.FOLD:
        player.isFolded = true;
        break;
      case PokerAction.CHECK:
        // Valid only if player.bet === updatedState.currentBet
        break;
      case PokerAction.CALL: {
        const callAmount = updatedState.currentBet - player.bet;
        player.chips -= callAmount;
        player.bet += callAmount;
        updatedState.pot += callAmount;
        break;
      }
      case PokerAction.RAISE: {
        const raiseAmount = message.data.amount ?? 0;
        const totalBet = player.bet + raiseAmount;
        player.chips -= raiseAmount;
        player.bet = totalBet;
        updatedState.pot += raiseAmount;
        updatedState.currentBet = totalBet;
        // Reset hasActed for other players
        updatedState.players.forEach((p) => {
          if (!p.isFolded && !p.isAllIn) p.hasActed = false;
        });
        break;
      }
      case PokerAction.ALL_IN: {
        const allInAmount = player.chips;
        player.bet += allInAmount;
        player.chips = 0;
        player.isAllIn = true;
        updatedState.pot += allInAmount;
        if (player.bet > updatedState.currentBet) {
          updatedState.currentBet = player.bet;
          // Reset hasActed for other players
          updatedState.players.forEach((p) => {
            if (!p.isFolded && !p.isAllIn) p.hasActed = false;
          });
        }
        break;
      }
    }

    player.hasActed = true;
    updatedState = this.advanceToNextPlayer(updatedState);

    // Check if betting round is over
    if (this.isBettingRoundOver(updatedState)) {
      updatedState = this.advanceBettingRound(updatedState);
    }

    return {
      updatedState,
      serverMessages: [{
        type: "game-state",
        roomId: gameState.roomId,
        data: updatedState,
      }],
    };
  }

  validateGameAction(
    gameState: PokerGameState,
    playerId: string,
    action: { action: PokerAction; amount?: number },
  ): boolean {
    const player = gameState.players[gameState.currentPlayerIndex];
    if (player.id !== playerId) return false;

    switch (action.action) {
      case PokerAction.CHECK:
        return player.bet === gameState.currentBet;
      case PokerAction.RAISE: {
        const raiseAmount = action.amount ?? 0;
        return (
          raiseAmount > 0 &&
          player.chips >= raiseAmount &&
          (player.bet + raiseAmount) >= (gameState.currentBet * 2)
        );
      }
      case PokerAction.CALL:
        return player.chips >= (gameState.currentBet - player.bet);
      default:
        return true; // FOLD and ALL_IN are always valid
    }
  }

  calculateScore(): number {
    return 0; // Not used in poker
  }

  override addPlayer(gameState: PokerGameState, player: PlayerState): PokerGameState {
    // Check if player already exists
    if (gameState.players.some((p) => p.id === player.id)) {
      return gameState;
    }

    // Create poker player with initialized state
    const pokerPlayer: PokerPlayer = {
      ...player,
      chips: gameState.settings.buyIn,
      cards: [],
      bet: 0,
      hasActed: false,
      isAllIn: false,
      isFolded: false,
      position: gameState.players.length,
    };

    return {
      ...gameState,
      players: [...gameState.players, pokerPlayer],
    };
  }

  override removePlayer(gameState: PokerGameState, playerId: string): PokerGameState {
    const updatedPlayers = gameState.players.filter((p) => p.id !== playerId);

    // Reindex positions
    const reindexedPlayers = updatedPlayers.map((p, i) => ({
      ...p,
      position: i,
    }));

    return {
      ...gameState,
      players: reindexedPlayers,
    };
  }

  private dealNewHand(gameState: PokerGameState): PokerGameState {
    let deck = shuffleDeck(createDeck());
    const players = gameState.players.map((p) => {
      const dealtCards = dealCards(deck, 2);
      deck = deck.slice(2);
      return {
        ...p,
        cards: dealtCards,
        bet: 0,
        hasActed: false,
        isFolded: false,
        isAllIn: false,
      };
    });

    const smallBlindPlayer = players[gameState.smallBlindIndex];
    const bigBlindPlayer = players[gameState.bigBlindIndex];

    smallBlindPlayer.bet = Math.min(
      smallBlindPlayer.chips,
      gameState.settings.smallBlind,
    );
    smallBlindPlayer.chips -= smallBlindPlayer.bet;

    bigBlindPlayer.bet = Math.min(
      bigBlindPlayer.chips,
      gameState.settings.bigBlind,
    );
    bigBlindPlayer.chips -= bigBlindPlayer.bet;

    return {
      ...gameState,
      phase: "playing",
      players,
      deck,
      communityCards: [],
      pot: smallBlindPlayer.bet + bigBlindPlayer.bet,
      currentBet: gameState.settings.bigBlind,
      bettingRound: BettingRound.PRE_FLOP,
      currentPlayerIndex: (gameState.bigBlindIndex + 1) % players.length,
      roundNumber: gameState.roundNumber + 1,
    };
  }

  private advanceToNextPlayer(gameState: PokerGameState): PokerGameState {
    let nextIndex = (gameState.currentPlayerIndex + 1) %
      gameState.players.length;
    while (
      gameState.players[nextIndex].isFolded ||
      gameState.players[nextIndex].isAllIn
    ) {
      nextIndex = (nextIndex + 1) % gameState.players.length;
    }
    return { ...gameState, currentPlayerIndex: nextIndex };
  }

  private isBettingRoundOver(gameState: PokerGameState): boolean {
    const activePlayers = gameState.players.filter(
      (p) => !p.isFolded && !p.isAllIn,
    );
    return activePlayers.every(
      (p) => p.hasActed && p.bet === gameState.currentBet,
    );
  }

  private advanceBettingRound(gameState: PokerGameState): PokerGameState {
    let { bettingRound, communityCards, deck } = gameState;

    // Reset player action status for the new round
    const players = gameState.players.map((p) => ({ ...p, hasActed: false }));

    switch (bettingRound) {
      case BettingRound.PRE_FLOP:
        bettingRound = BettingRound.FLOP;
        communityCards = dealCards(deck, 3);
        deck = deck.slice(3);
        break;
      case BettingRound.FLOP:
        bettingRound = BettingRound.TURN;
        communityCards.push(...dealCards(deck, 1));
        deck = deck.slice(1);
        break;
      case BettingRound.TURN:
        bettingRound = BettingRound.RIVER;
        communityCards.push(...dealCards(deck, 1));
        deck = deck.slice(1);
        break;
      case BettingRound.RIVER:
        return this.determineWinner(gameState);
    }

    return {
      ...gameState,
      players,
      bettingRound,
      communityCards,
      deck,
      currentPlayerIndex: (gameState.smallBlindIndex) %
        gameState.players.length,
      currentBet: 0,
    };
  }

  private determineWinner(gameState: PokerGameState): PokerGameState {
    const activePlayers = gameState.players.filter((p) => !p.isFolded);
    if (activePlayers.length === 1) {
      const winner = activePlayers[0];
      winner.chips += gameState.pot;
      // End of hand, prepare for next
      return this.endHand(gameState, [winner.id]);
    }

    const hands = activePlayers.map((player) => ({
      playerId: player.id,
      hand: evaluateHand([...player.cards, ...gameState.communityCards]),
    }));

    hands.sort((a, b) => compareHands(b.hand, a.hand));

    const winners = [hands[0].playerId];
    for (let i = 1; i < hands.length; i++) {
      if (compareHands(hands[0].hand, hands[i].hand) === 0) {
        winners.push(hands[i].playerId);
      } else {
        break;
      }
    }

    const potPerWinner = gameState.pot / winners.length;
    winners.forEach((winnerId) => {
      const winnerPlayer = gameState.players.find((p) => p.id === winnerId);
      if (winnerPlayer) {
        winnerPlayer.chips += potPerWinner;
      }
    });

    return this.endHand(gameState, winners);
  }

  private endHand(
    gameState: PokerGameState,
    winners: string[],
  ): PokerGameState {
    // In a real game, you'd have a delay here to show results
    const nextSmallBlindIndex = (gameState.smallBlindIndex + 1) %
      gameState.players.length;
    const nextBigBlindIndex = (gameState.bigBlindIndex + 1) %
      gameState.players.length;

    const newGameState: PokerGameState = {
      ...gameState,
      phase: "finished",
      smallBlindIndex: nextSmallBlindIndex,
      bigBlindIndex: nextBigBlindIndex,
    };

    // Announce winner(s)
    const _serverMessages: PokerServerMessage[] = [{
      type: "hand-result",
      roomId: gameState.roomId,
      data: {
        winners,
        pot: gameState.pot,
      },
    }];

    // This is a simplified approach. A full implementation would need to handle
    // the state transition more gracefully, perhaps with a "results" phase.
    // For now, we'll just reset for the next hand immediately.
    return this.dealNewHand(newGameState);
  }
}
