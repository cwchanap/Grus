import { Handlers } from "$fresh/server.ts";
import { getSession } from "../../../lib/auth/auth-utils.ts";
import { getPrismaClient } from "../../../lib/auth/prisma-client.ts";
import { getConfig } from "../../../lib/config.ts";

export const handler: Handlers = {
  async POST(req) {
    try {
      const body = await req.json();
      const { name } = body;

      // Validate input
      if (typeof name !== "string") {
        return new Response(
          JSON.stringify({ error: "Display name must be a string" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      // Validate name length (allow empty string to clear name)
      if (name.length > 100) {
        return new Response(
          JSON.stringify({ error: "Display name must be 100 characters or less" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      // Get current user from session
      const config = getConfig();
      const cookie = req.headers.get("Cookie");
      const cookieName = config.auth.sessionCookie.name;
      const token = cookie?.split(";")
        .find((c) => c.trim().startsWith(`${cookieName}=`))
        ?.split("=")[1];

      if (!token) {
        return new Response(
          JSON.stringify({ error: "Not authenticated" }),
          { status: 401, headers: { "Content-Type": "application/json" } },
        );
      }

      const session = await getSession(token);
      if (!session) {
        return new Response(
          JSON.stringify({ error: "Invalid or expired session" }),
          { status: 401, headers: { "Content-Type": "application/json" } },
        );
      }

      const prisma = await getPrismaClient();

      // Update user's display name
      const updatedUser = await prisma.user.update({
        where: { id: session.user.id },
        data: { name: name.trim() || null }, // Store null for empty strings
        select: {
          id: true,
          email: true,
          username: true,
          name: true,
        },
      });

      return new Response(
        JSON.stringify({
          success: true,
          message: "Display name updated successfully",
          user: updatedUser,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    } catch (error) {
      console.error("Update profile error:", error);
      return new Response(
        JSON.stringify({ error: "Internal server error" }),
        { status: 500, headers: { "Content-Type": "application/json" } },
      );
    }
  },
};
