/// <reference lib="deno.unstable" />

// KV-backed Room Database Service (async)
// Implements room, player, session, and score operations using Deno KV.
// Key structure:
// - ["room", roomId] => Room
// - ["player", playerId] => Player
// - ["room_players", roomId, playerId] => { joinedAt }
// - ["session", sessionId] => GameSession
// - ["session_scores", sessionId, scoreId] => Score
// - ["room_sessions", roomId, sessionId] => true
// Listing rooms uses prefix scan on ["room"] and sorting by createdAt desc.

import type { Player, Room } from "../../types/core/room.ts";
import type { Score } from "../../types/core/game.ts";

export interface DatabaseResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

type GameSession = {
  id: string;
  roomId: string;
  gameType: string;
  totalRounds: number;
  startedAt: string; // ISO
  endedAt?: string; // ISO
  winnerId?: string;
};

export class KVRoomService {
  private kv: Deno.Kv | null = null;

  private async getKv(): Promise<Deno.Kv> {
    if (!this.kv) this.kv = await Deno.openKv();
    return this.kv;
  }

  private async op<T>(fn: () => Promise<T>, msg: string): Promise<DatabaseResult<T>> {
    try {
      const data = await fn();
      return { success: true, data };
    } catch (err) {
      console.error(`KVRoomService error: ${msg}`, err);
      return { success: false, error: err instanceof Error ? err.message : msg };
    }
  }

  private generateId(): string {
    return crypto.randomUUID();
  }

  // Rooms
  async createRoom(
    name: string,
    hostId: string | null = null,
    maxPlayers = 8,
    gameType = "drawing",
    isPrivate = false,
  ): Promise<DatabaseResult<string>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const id = this.generateId();
      const now = new Date().toISOString();
      const room: Room = {
        id,
        name,
        hostId,
        maxPlayers,
        gameType,
        isPrivate,
        isActive: true,
        createdAt: now,
        updatedAt: now,
      };
      await kv.set(["room", id], room);
      return id;
    }, "Failed to create room");
  }

  async getRoomById(id: string): Promise<DatabaseResult<Room | null>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const res = await kv.get<Room>(["room", id]);
      return (res.value as Room | null) ?? null;
    }, `Failed to get room ${id}`);
  }

  async getAllRooms(limit?: number): Promise<DatabaseResult<Room[]>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const iter = kv.list<Room>({ prefix: ["room"] });
      const rooms: Room[] = [];
      for await (const entry of iter) {
        if (entry.value) rooms.push(entry.value as Room);
      }
      rooms.sort((a, b) => (b.createdAt > a.createdAt ? 1 : -1));
      return typeof limit === "number" ? rooms.slice(0, limit) : rooms;
    }, "Failed to list rooms");
  }

  async getActiveRooms(limit = 20): Promise<DatabaseResult<Room[]>> {
    return await this.op(async () => {
      const all = (await this.getAllRooms()).data ?? [];
      const active = all.filter((r) => r.isActive && !r.isPrivate);
      return active.slice(0, limit);
    }, "Failed to list active rooms");
  }

  async updateRoom(id: string, updates: Partial<Room>): Promise<DatabaseResult<boolean>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const res = await kv.get<Room>(["room", id]);
      if (!res.value) throw new Error("Room not found");
      const now = new Date().toISOString();
      const updated: Room = { ...(res.value as Room), ...updates, updatedAt: now };
      await kv.set(["room", id], updated);
      return true;
    }, `Failed to update room ${id}`);
  }

  async deleteRoom(id: string): Promise<DatabaseResult<boolean>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      // Delete room
      await kv.delete(["room", id]);
      // Delete player index entries
      const idxIter = kv.list({ prefix: ["room_players", id] });
      for await (const entry of idxIter) {
        await kv.delete(entry.key);
      }
      // Optionally, clean sessions mapping
      const sessIter = kv.list({ prefix: ["room_sessions", id] });
      for await (const entry of sessIter) {
        await kv.delete(entry.key);
      }
      return true;
    }, `Failed to delete room ${id}`);
  }

  async deleteAllRooms(): Promise<DatabaseResult<number>> {
    return await this.op(async () => {
      const rooms = (await this.getAllRooms()).data ?? [];
      for (const r of rooms) {
        await this.deleteRoom(r.id);
      }
      return rooms.length;
    }, "Failed to delete all rooms");
  }

  // Players
  async createPlayer(
    name: string,
    roomId: string,
    isHost = false,
  ): Promise<DatabaseResult<string>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const id = this.generateId();
      const now = new Date().toISOString();
      const player: Player = { id, name, roomId, isHost, joinedAt: now };
      const atomic = kv.atomic()
        .set(["player", id], player)
        .set(["room_players", roomId, id], { joinedAt: now });
      const ok = await atomic.commit();
      if (!ok.ok) throw new Error("Atomic createPlayer failed");
      return id;
    }, "Failed to create player");
  }

  async getPlayerById(id: string): Promise<DatabaseResult<Player | null>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const res = await kv.get<Player>(["player", id]);
      return (res.value as Player | null) ?? null;
    }, `Failed to get player ${id}`);
  }

  async getPlayersByRoom(roomId: string): Promise<DatabaseResult<Player[]>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const idx = kv.list({ prefix: ["room_players", roomId] });
      const players: Player[] = [];
      for await (const entry of idx) {
        const pid = String(entry.key[2]);
        const p = await kv.get<Player>(["player", pid]);
        if (p.value) players.push(p.value as Player);
      }
      // Order by joinedAt asc
      players.sort((a, b) => (a.joinedAt < b.joinedAt ? -1 : 1));
      return players;
    }, `Failed to get players for room ${roomId}`);
  }

  async updatePlayer(
    id: string,
    updates: Partial<{ name: string; isHost: boolean }>,
  ): Promise<DatabaseResult<boolean>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const res = await kv.get<Player>(["player", id]);
      if (!res.value) throw new Error("Player not found");
      const current = res.value as Player;
      const updated: Player = {
        ...current,
        name: updates.name ?? current.name,
        isHost: updates.isHost ?? current.isHost,
      };
      await kv.set(["player", id], updated);
      return true;
    }, `Failed to update player ${id}`);
  }

  async removePlayer(id: string): Promise<DatabaseResult<boolean>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const playerRes = await kv.get<Player>(["player", id]);
      if (!playerRes.value) return true; // already gone
      const player = playerRes.value as Player;
      // Guard: if player has no roomId, just delete the player record
      if (!player.roomId) {
        await kv.delete(["player", id]);
        return true;
      }
      const roomId = player.roomId; // narrowed to string
      const atomic = kv.atomic()
        .delete(["player", id])
        .delete(["room_players", roomId, id]);
      const ok = await atomic.commit();
      if (!ok.ok) throw new Error("Atomic removePlayer failed");
      return true;
    }, `Failed to remove player ${id}`);
  }

  // Sessions
  async createGameSession(
    roomId: string,
    gameType: string,
    totalRounds = 5,
  ): Promise<DatabaseResult<string>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const id = this.generateId();
      const session: GameSession = {
        id,
        roomId,
        gameType,
        totalRounds,
        startedAt: new Date().toISOString(),
      };
      const atomic = kv.atomic()
        .set(["session", id], session)
        .set(["room_sessions", roomId, id], true);
      const ok = await atomic.commit();
      if (!ok.ok) throw new Error("Atomic createGameSession failed");
      return id;
    }, "Failed to create game session");
  }

  async endGameSession(id: string, winnerId?: string): Promise<DatabaseResult<boolean>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const res = await kv.get<GameSession>(["session", id]);
      if (!res.value) throw new Error("Session not found");
      const updated: GameSession = {
        ...(res.value as GameSession),
        endedAt: new Date().toISOString(),
        winnerId,
      };
      await kv.set(["session", id], updated);
      return true;
    }, `Failed to end session ${id}`);
  }

  // Scores
  async createScore(
    sessionId: string,
    playerId: string,
    points = 0,
  ): Promise<DatabaseResult<string>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const id = this.generateId();
      const score: Score = {
        id,
        sessionId,
        playerId,
        points,
        gameSpecificData: { correctGuesses: 0 },
      } as Score;
      const atomic = kv.atomic()
        .set(["session_scores", sessionId, id], score)
        .set(["score_index", id], sessionId);
      const ok = await atomic.commit();
      if (!ok.ok) throw new Error("Atomic createScore failed");
      return id;
    }, "Failed to create score");
  }

  async updateScore(
    id: string,
    points: number,
    correctGuesses: number,
  ): Promise<DatabaseResult<boolean>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      // Need sessionId to locate the score; scan all sessions may be heavy.
      // We'll index by a global mapping as fallback: ["score_index", scoreId] => sessionId
      const idx = await kv.get<string>(["score_index", id]);
      const sessionId = idx.value as string | undefined;
      if (!sessionId) throw new Error("Score index not found");
      const sid: string = sessionId; // Narrow to non-undefined
      const scoreRes = await kv.get<Score>(["session_scores", sid, id]);
      if (!scoreRes.value) throw new Error("Score not found");
      const current = scoreRes.value as Score;
      const updated: Score = {
        ...current,
        points,
        gameSpecificData: { ...(current.gameSpecificData || {}), correctGuesses },
      };
      await kv.set(["session_scores", sid, id], updated);
      return true;
    }, `Failed to update score ${id}`);
  }

  async getScoresBySession(sessionId: string): Promise<DatabaseResult<Score[]>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const iter = kv.list<Score>({ prefix: ["session_scores", sessionId] });
      const scores: Score[] = [];
      for await (const entry of iter) {
        if (entry.value) scores.push(entry.value as Score);
      }
      // Sort by points desc
      scores.sort((a, b) => b.points - a.points);
      return scores;
    }, `Failed to get scores for session ${sessionId}`);
  }

  async healthCheck(): Promise<DatabaseResult<boolean>> {
    return await this.op(async () => {
      const kv = await this.getKv();
      const key = ["health", "ping"] as const;
      await kv.set(key, { ts: Date.now() }, { expireIn: 1000 * 10 });
      const got = await kv.get(key);
      await kv.delete(key);
      return got.value !== null;
    }, "KV health check failed");
  }

  close(): void {
    if (this.kv) {
      this.kv.close();
      this.kv = null;
    }
  }
}

let kvRoomService: KVRoomService | null = null;
export function getKVRoomService(): KVRoomService {
  if (!kvRoomService) kvRoomService = new KVRoomService();
  return kvRoomService;
}
