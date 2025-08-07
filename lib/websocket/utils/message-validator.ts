// Message validation utilities
import { validateChatMessage, validatePlayerName } from "../../config.ts";
import type { BaseClientMessage } from "../../../types/core/websocket.ts";

export class MessageValidator {
  validateClientMessage(message: unknown): message is BaseClientMessage {
    return (
      message !== null &&
      typeof message === "object" &&
      "type" in message &&
      "roomId" in message &&
      "playerId" in message &&
      "data" in message &&
      typeof (message as any).type === "string" &&
      typeof (message as any).roomId === "string" &&
      typeof (message as any).playerId === "string" &&
      (message as any).data !== undefined
    );
  }

  validateDrawingCommand(data: unknown): boolean {
    return (
      data !== null &&
      typeof data === "object" &&
      "type" in data &&
      typeof (data as any).type === "string" &&
      ["start", "move", "end", "clear"].includes((data as any).type) &&
      ((data as any).type === "clear" ||
        ("x" in data &&
          "y" in data &&
          typeof (data as any).x === "number" &&
          typeof (data as any).y === "number" &&
          (data as any).x >= 0 &&
          (data as any).x <= 1920 && // Max canvas width
          (data as any).y >= 0 &&
          (data as any).y <= 1080)) // Max canvas height
    );
  }

  validatePlayerName(playerName: string): boolean {
    return validatePlayerName(playerName);
  }

  validateChatMessage(message: string): boolean {
    return validateChatMessage(message);
  }

  validateRoomId(roomId: string): boolean {
    return typeof roomId === "string" &&
      roomId.length === 6 &&
      /^[A-Z0-9]{6}$/.test(roomId);
  }

  validatePlayerId(playerId: string): boolean {
    return typeof playerId === "string" &&
      playerId.length > 0 &&
      playerId.length <= 100;
  }
}
