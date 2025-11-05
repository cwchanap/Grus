// Use dynamic import to avoid module resolution issues
let prisma: any = null;

export async function getPrismaClient(): Promise<any> {
  if (!prisma) {
    const connectionString = Deno.env.get("DATABASE_URL");

    if (!connectionString) {
      throw new Error("DATABASE_URL environment variable is required");
    }

    try {
      // Dynamic import to avoid module resolution issues
      const prismaModule = await import("@prisma/client") as any;
      const PrismaClient = (prismaModule as any).PrismaClient ||
        (prismaModule as any).default?.PrismaClient ||
        (prismaModule as any).default;

      if (!PrismaClient) {
        throw new Error("PrismaClient not found in the imported module");
      }

      // Initialize Prisma client
      prisma = new PrismaClient({
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
