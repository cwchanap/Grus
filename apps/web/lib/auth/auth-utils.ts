import { jwtVerify, SignJWT } from "jose";
import { getPrismaClient } from "./prisma-client.ts";
import { getConfig } from "../config.ts";

// JWT secret is optional - only required if authentication features are used
const JWT_SECRET_STRING = Deno.env.get("JWT_SECRET");
const JWT_SECRET = JWT_SECRET_STRING ? new TextEncoder().encode(JWT_SECRET_STRING) : null;

function ensureJWTSecret(): Uint8Array {
  if (!JWT_SECRET) {
    throw new Error(
      "JWT_SECRET environment variable is required for authentication. " +
        "Please set JWT_SECRET to a secure random string (>32 characters). " +
        "Authentication features are optional - the game can run without them.",
    );
  }
  return JWT_SECRET;
}

// Local User type definition (matches Prisma User schema)
export interface User {
  id: string;
  email: string;
  username: string;
  password: string;
  name: string | null;
  avatar?: string | null;
  createdAt?: Date;
  updatedAt?: Date;
}

export interface UserPayload {
  id: string;
  email: string;
  username: string;
  name: string | null;
  avatar?: string | null;
}

export interface SessionData {
  user: UserPayload;
  token: string;
  expiresAt: Date;
}

// Parse JWT expiration time string to milliseconds
function parseExpirationTime(expiresIn: string): number {
  const match = expiresIn.match(/^(\d+)([dhms])$/);
  if (!match) throw new Error(`Invalid expiration time format: ${expiresIn}`);

  const value = parseInt(match[1]);
  const unit = match[2];

  switch (unit) {
    case "d":
      return value * 24 * 60 * 60 * 1000; // days to ms
    case "h":
      return value * 60 * 60 * 1000; // hours to ms
    case "m":
      return value * 60 * 1000; // minutes to ms
    case "s":
      return value * 1000; // seconds to ms
    default:
      throw new Error(`Invalid time unit: ${unit}`);
  }
}

// Hash password using Web Crypto API
export async function hashPassword(password: string): Promise<string> {
  const encoder = new TextEncoder();
  const data = encoder.encode(password);
  const hash = await crypto.subtle.digest("SHA-256", data);
  const hashArray = Array.from(new Uint8Array(hash));
  return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
}

// Verify password
export async function verifyPassword(password: string, hashedPassword: string): Promise<boolean> {
  const hashedInput = await hashPassword(password);
  return hashedInput === hashedPassword;
}

// Generate JWT token
export async function generateToken(user: UserPayload): Promise<string> {
  const config = getConfig();
  const secret = ensureJWTSecret();
  const token = await new SignJWT({ ...user })
    .setProtectedHeader({ alg: "HS256" })
    .setIssuedAt()
    .setExpirationTime(config.auth.jwtExpiresIn)
    .sign(secret);

  return token;
}

// Verify JWT token
export async function verifyToken(token: string): Promise<UserPayload | null> {
  try {
    const secret = ensureJWTSecret();
    const { payload } = await jwtVerify(token, secret);
    return payload as unknown as UserPayload;
  } catch {
    return null;
  }
}

// Create user
export async function createUser(
  email: string,
  username: string,
  password: string,
  name?: string,
): Promise<User> {
  const prisma = await getPrismaClient();

  const hashedPassword = await hashPassword(password);

  return await prisma.user.create({
    data: {
      email,
      username,
      password: hashedPassword,
      name,
    },
  });
}

// Create user session
export async function createSession(user: User): Promise<SessionData> {
  const prisma = await getPrismaClient();
  const config = getConfig();

  const userPayload: UserPayload = {
    id: user.id,
    email: user.email,
    username: user.username,
    name: user.name,
    avatar: user.avatar ?? null,
  };

  const token = await generateToken(userPayload);
  const expirationMs = parseExpirationTime(config.auth.jwtExpiresIn);
  const expiresAt = new Date(Date.now() + expirationMs);

  // Store session in database
  await prisma.session.create({
    data: {
      token,
      userId: user.id,
      expiresAt,
    },
  });

  return {
    user: userPayload,
    token,
    expiresAt,
  };
}

// Get session from token
export async function getSession(token: string): Promise<SessionData | null> {
  const prisma = await getPrismaClient();

  const session = await prisma.session.findUnique({
    where: { token },
    include: { user: true },
  });

  if (!session || session.expiresAt < new Date()) {
    return null;
  }

  const userPayload: UserPayload = {
    id: session.user.id,
    email: session.user.email,
    username: session.user.username,
    name: session.user.name,
    avatar: session.user.avatar ?? null,
  };

  return {
    user: userPayload,
    token: session.token,
    expiresAt: session.expiresAt,
  };
}

// Delete session
export async function deleteSession(token: string): Promise<void> {
  const prisma = await getPrismaClient();

  await prisma.session.delete({
    where: { token },
  }).catch(() => {
    // Ignore if session doesn't exist
  });
}

// Clean up expired sessions
export async function cleanupExpiredSessions(): Promise<void> {
  const prisma = await getPrismaClient();

  await prisma.session.deleteMany({
    where: {
      expiresAt: {
        lt: new Date(),
      },
    },
  });
}
