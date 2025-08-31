// Application configuration - hardcoded settings instead of env variables

export interface AppConfig {
  environment: "development" | "production";
  auth: {
    jwtExpiresIn: string;
    sessionCookie: {
      name: string;
      secure: boolean;
      httpOnly: boolean;
      sameSite: "Strict" | "Lax" | "None";
    };
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
  drawing: {
    serverDebounceMs: number; // milliseconds to batch drawing commands on server
    maxBatchSize: number; // maximum commands to batch before forcing flush
  };
}

export function getConfig(): AppConfig {
  const isDev = Deno.env.get("DENO_ENV") === "development";

  return {
    environment: isDev ? "development" : "production",
    auth: {
      jwtExpiresIn: "7d", // 7 days
      sessionCookie: {
        name: "grus_session",
        secure: !isDev, // false in dev, true in production
        httpOnly: true,
        sameSite: "Lax",
      },
    },
    game: {
      maxPlayersPerRoom: 8,
      roundTimeLimit: 120, // 2 minutes
      maxRooms: isDev ? 10 : 1000,
      chatMessageLimit: 100,
    },
    websocket: {
      maxConnections: isDev ? 100 : 10000,
      heartbeatInterval: 30000, // 30 seconds
      connectionTimeout: 60000, // 1 minute
    },
    security: {
      rateLimitMessages: 30, // 30 messages per minute
      rateLimitDrawing: 60, // 60 drawing actions per second
      maxMessageLength: 200,
      maxPlayerNameLength: 20,
    },
    drawing: {
      serverDebounceMs: 500, // 500ms batching for drawing commands
      maxBatchSize: 20, // maximum 20 commands per batch
    },
  };
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

// Parse JWT expiration time string to seconds for cookie Max-Age
export function parseExpirationTimeToSeconds(expiresIn: string): number {
  const match = expiresIn.match(/^(\d+)([dhms])$/);
  if (!match) throw new Error(`Invalid expiration time format: ${expiresIn}`);

  const value = parseInt(match[1]);
  const unit = match[2];

  switch (unit) {
    case "d":
      return value * 24 * 60 * 60; // days to seconds
    case "h":
      return value * 60 * 60; // hours to seconds
    case "m":
      return value * 60; // minutes to seconds
    case "s":
      return value; // seconds
    default:
      throw new Error(`Invalid time unit: ${unit}`);
  }
}
