// Environment configuration for development and production

export interface AppConfig {
  environment: "development" | "production";
  database: {
    name: string;
    migrationPath: string;
  };
  kv: {
    namespace: string;
    defaultTtl: number;
  };
  game: {
    maxPlayersPerRoom: number;
    roundTimeLimit: number; // seconds
    maxRooms: number;
    chatMessageLimit: number;
  };
  websocket: {
    maxConnections: number;
    heartbeatInterval: number; // milliseconds
    connectionTimeout: number; // milliseconds
  };
  security: {
    rateLimitMessages: number; // per minute
    rateLimitDrawing: number; // per second
    maxMessageLength: number;
    maxPlayerNameLength: number;
  };
}

export function getConfig(env?: string): AppConfig {
  let environment: "development" | "production";
  try {
    environment = (env || Deno.env.get("ENVIRONMENT") || "development") as
      | "development"
      | "production";
  } catch {
    environment = "development";
  }

  // Helper function to get env var with fallback
  const getEnvVar = (key: string, fallback: string | number): string | number => {
    try {
      const value = Deno.env.get(key);
      if (value === undefined) return fallback;
      return typeof fallback === "number" ? parseInt(value) || fallback : value;
    } catch {
      return fallback;
    }
  };

  const baseConfig: AppConfig = {
    environment,
    database: {
      name: getEnvVar(
        "DATABASE_NAME",
        environment === "production" ? "drawing-game-db" : "drawing-game-db-dev",
      ) as string,
      migrationPath: "./db/migrations",
    },
    kv: {
      namespace: environment === "production" ? "GAME_STATE" : "GAME_STATE_DEV",
      defaultTtl: 3600, // 1 hour
    },
    game: {
      maxPlayersPerRoom: getEnvVar("MAX_PLAYERS_PER_ROOM", 8) as number,
      roundTimeLimit: getEnvVar("ROUND_TIME_LIMIT", 120) as number, // 2 minutes
      maxRooms: getEnvVar("MAX_ROOMS", environment === "production" ? 1000 : 10) as number,
      chatMessageLimit: 100,
    },
    websocket: {
      maxConnections: getEnvVar(
        "MAX_CONNECTIONS",
        environment === "production" ? 10000 : 100,
      ) as number,
      heartbeatInterval: getEnvVar("HEARTBEAT_INTERVAL", 30000) as number, // 30 seconds
      connectionTimeout: getEnvVar("CONNECTION_TIMEOUT", 60000) as number, // 1 minute
    },
    security: {
      rateLimitMessages: getEnvVar("RATE_LIMIT_MESSAGES", 30) as number, // 30 messages per minute
      rateLimitDrawing: getEnvVar("RATE_LIMIT_DRAWING", 60) as number, // 60 drawing actions per second
      maxMessageLength: getEnvVar("MAX_MESSAGE_LENGTH", 200) as number,
      maxPlayerNameLength: getEnvVar("MAX_PLAYER_NAME_LENGTH", 20) as number,
    },
  };

  return baseConfig;
}

// Validation functions
export function validatePlayerName(name: string): boolean {
  const config = getConfig();
  return name.length > 0 &&
    name.length <= config.security.maxPlayerNameLength &&
    /^[a-zA-Z0-9\s_-]+$/.test(name);
}

export function validateChatMessage(message: string): boolean {
  const config = getConfig();
  return message.length > 0 &&
    message.length <= config.security.maxMessageLength;
}

export function validateRoomName(name: string): boolean {
  return name.length > 0 &&
    name.length <= 50 &&
    /^[a-zA-Z0-9\s_-]+$/.test(name);
}

// Utility functions
export function generateId(): string {
  return crypto.randomUUID();
}

export function generateRoomCode(): string {
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  let result = "";
  for (let i = 0; i < 6; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}
