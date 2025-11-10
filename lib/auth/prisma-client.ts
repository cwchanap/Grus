import { PrismaClient } from "../../generated/prisma/client.ts";

let prisma: PrismaClient | null = null;

export function getPrismaClient(): PrismaClient {
  if (!prisma) {
    const connectionString = Deno.env.get("DATABASE_URL");

    if (!connectionString) {
      throw new Error("DATABASE_URL environment variable is required");
    }

    prisma = new PrismaClient({
      log: ["error", "warn"],
    });
  }

  return prisma;
}

export async function closePrismaClient(): Promise<void> {
  if (prisma) {
    await prisma.$disconnect();
    prisma = null;
  }
}
