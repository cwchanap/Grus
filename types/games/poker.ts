import { BaseGameSettings, BaseGameState } from "../core/game.ts";
import { BaseClientMessage, BaseServerMessage } from "../core/websocket.ts";
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
  status?: "active" | "folded" | "all-in";
  isDealer?: boolean;
}

export interface PokerGameSettings extends BaseGameSettings {
  buyIn: number;
  smallBlind: number;
  bigBlind: number;
}

export interface PokerGameState extends BaseGameState {
  players: PokerPlayer[];
  deck: PokerCard[];
  communityCards: PokerCard[];
  pot: number;
  currentBet: number;
  minRaise: number;
  bettingRound: BettingRound;
  currentPlayerIndex: number;
  currentPlayerId?: string;
  smallBlindIndex: number;
  bigBlindIndex: number;
  settings: PokerGameSettings;
}

// Messages
export interface PokerClientMessage extends BaseClientMessage {
  type:
    | "poker-action"
    | "game-action"
    | "join-room"
    | "leave-room"
    | "chat"
    | "start-game"
    | "end-game"
    | "update-settings"
    | "ping";
  data: {
    action: PokerAction;
    amount?: number;
  };
}

export interface PokerServerMessage extends BaseServerMessage {
  type:
    | "game-state"
    | "hand-result"
    | "room-update"
    | "chat-message"
    | "score-update"
    | "host-changed"
    | "settings-updated"
    | "error"
    | "pong";
  data: PokerGameState | {
    winners?: string[];
    pot?: number;
  };
}
