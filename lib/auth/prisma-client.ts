// Use dynamic import to avoid module resolution issues
// Define expected structure of the Prisma module
interface PrismaModule {
  PrismaClient?: unknown;
  default?: {
    PrismaClient?: unknown;
  } | unknown;
}

// deno-lint-ignore no-explicit-any
let prisma: any = null;

// deno-lint-ignore no-explicit-any
export async function getPrismaClient(): Promise<any> {
  if (!prisma) {
    const connectionString = Deno.env.get("DATABASE_URL");

    if (!connectionString) {
      throw new Error("DATABASE_URL environment variable is required");
    }

    try {
      // Dynamic import to avoid module resolution issues
      const prismaModule = await import("@prisma/client") as PrismaModule;

      // Try multiple access patterns to find PrismaClient
      const PrismaClient = prismaModule.PrismaClient ||
        (prismaModule.default &&
            typeof prismaModule.default === "object" &&
            "PrismaClient" in prismaModule.default
          ? (prismaModule.default as { PrismaClient: unknown }).PrismaClient
          : prismaModule.default);

      if (!PrismaClient) {
        throw new Error("PrismaClient not found in the imported module");
      }

      // Initialize Prisma client
      // deno-lint-ignore no-explicit-any
      prisma = new (PrismaClient as any)({
        log: ["error", "warn"],
      });
    } catch (error) {
      console.error("Failed to load Prisma client:", error);
      throw new Error("Prisma client not available. Please run 'npx prisma generate' first.");
    }
  }

  return prisma;
}

export async function closePrismaClient(): Promise<void> {
  if (prisma) {
    await prisma.$disconnect();
    prisma = null;
  }
}
