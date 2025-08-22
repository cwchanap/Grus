// Use dynamic import to avoid module resolution issues
let PrismaClient: any;
let prisma: any = null;

export async function getPrismaClient(): Promise<any> {
  if (!prisma) {
    const connectionString = Deno.env.get("DATABASE_URL");

    if (!connectionString) {
      throw new Error("DATABASE_URL environment variable is required");
    }

    try {
      // Dynamic import to avoid module resolution issues
      const { PrismaClient: PC } = await import("npm:@prisma/client@5.18.0");
      PrismaClient = PC;

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
