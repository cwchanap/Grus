import { BaseGameSettings, BaseGameState } from "../core/game.ts";
import { PlayerState } from "../core/room.ts";

export enum Suit {
  Hearts = "H",
  Diamonds = "D",
  Clubs = "C",
  Spades = "S",
}

export enum Rank {
  Two = "2",
  Three = "3",
  Four = "4",
  Five = "5",
  Six = "6",
  Seven = "7",
  Eight = "8",
  Nine = "9",
  Ten = "T",
  Jack = "J",
  Queen = "Q",
  King = "K",
  Ace = "A",
}

export interface PokerCard {
  suit: Suit;
  rank: Rank;
}

export enum PokerAction {
  FOLD = "fold",
  CHECK = "check",
  CALL = "call",
  RAISE = "raise",
  ALL_IN = "all-in",
}

export enum BettingRound {
  PRE_FLOP = "Pre-flop",
  FLOP = "Flop",
  TURN = "Turn",
  RIVER = "River",
  SHOWDOWN = "Showdown",
}

export enum HandRank {
  HIGH_CARD,
  ONE_PAIR,
  TWO_PAIR,
  THREE_OF_A_KIND,
  STRAIGHT,
  FLUSH,
  FULL_HOUSE,
  FOUR_OF_A_KIND,
  STRAIGHT_FLUSH,
  ROYAL_FLUSH,
}

export interface PokerHand {
  rank: HandRank;
  values: number[];
}

export interface PokerPlayer extends PlayerState {
  chips: number;
  cards: PokerCard[];
  bet: number;
  hasActed: boolean;
  isAllIn: boolean;
  isFolded: boolean;
  position: number;
}

export interface PokerGameSettings extends BaseGameSettings {
  buyIn: number;
  smallBlind: number;
  bigBlind: number;
}

export interface PokerGameState extends BaseGameState<PokerGameSettings, Record<string, never>> {
  gameType: "poker";
  players: PokerPlayer[];
  deck: PokerCard[];
  communityCards: PokerCard[];
  pot: number;
  currentBet: number;
  minRaise: number;
  bettingRound: BettingRound;
  currentPlayerIndex: number;
  smallBlindIndex: number;
  bigBlindIndex: number;
  settings: PokerGameSettings;
}

// Messages - Discriminated union for poker client messages
export type PokerClientMessage =
  | {
    type: "poker-action" | "game-action";
    roomId: string;
    playerId: string;
    data: {
      action: PokerAction;
      amount?: number;
    };
  }
  | {
    type: "chat";
    roomId: string;
    playerId: string;
    data: {
      text: string;
      playerName?: string;
    };
  }
  | {
    type: "join-room";
    roomId: string;
    playerId: string;
    data: {
      playerName: string;
    };
  }
  | {
    type: "leave-room";
    roomId: string;
    playerId: string;
    data: Record<string, never>; // Empty object
  }
  | {
    type: "start-game";
    roomId: string;
    playerId: string;
    data: Record<string, never>; // Empty object
  }
  | {
    type: "end-game";
    roomId: string;
    playerId: string;
    data: Record<string, never>; // Empty object
  }
  | {
    type: "update-settings";
    roomId: string;
    playerId: string;
    data: {
      settings: Partial<PokerGameSettings>;
    };
  }
  | {
    type: "ping";
    roomId: string;
    playerId: string;
    data: Record<string, never>; // Empty object
  };

// Discriminated union for poker server messages
export type PokerServerMessage =
  | {
    type: "game-state";
    roomId: string;
    data: PokerGameState;
  }
  | {
    type: "hand-result";
    roomId: string;
    data: {
      winners: string[];
      pot: number;
    };
  }
  | {
    type: "room-update";
    roomId: string;
    data: {
      players: PlayerState[];
      hostId?: string;
    };
  }
  | {
    type: "chat-message";
    roomId: string;
    data: {
      id: string;
      playerId: string;
      playerName: string;
      message: string;
      timestamp: number;
      isSystemMessage?: boolean;
    };
  }
  | {
    type: "score-update";
    roomId: string;
    data: {
      scores: Record<string, number>;
      sessionId?: string;
    };
  }
  | {
    type: "host-changed";
    roomId: string;
    data: {
      newHostId: string;
      newHostName?: string;
    };
  }
  | {
    type: "settings-updated";
    roomId: string;
    data: PokerGameSettings;
  }
  | {
    type: "error";
    roomId: string;
    data: {
      error: string;
      message?: string;
      code?: string;
    };
  }
  | {
    type: "pong";
    roomId: string;
    data: {
      timestamp: number;
    };
  };
