import { Handlers } from "$fresh/server.ts";
import { getSession, hashPassword, verifyPassword } from "../../../lib/auth/auth-utils.ts";
import { getPrismaClient } from "../../../lib/auth/prisma-client.ts";
import { getConfig } from "../../../lib/config.ts";

export const handler: Handlers = {
  async POST(req) {
    try {
      const body = await req.json();
      const { currentPassword, newPassword } = body;

      // Validate input
      if (!currentPassword || !newPassword) {
        return new Response(
          JSON.stringify({ error: "Current password and new password are required" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      // Validate new password length
      if (newPassword.length < 6) {
        return new Response(
          JSON.stringify({ error: "New password must be at least 6 characters" }),
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

      // Get user from database
      const user = await prisma.user.findUnique({
        where: { id: session.user.id },
      });

      if (!user) {
        return new Response(
          JSON.stringify({ error: "User not found" }),
          { status: 404, headers: { "Content-Type": "application/json" } },
        );
      }

      // Verify current password
      const isCurrentPasswordValid = await verifyPassword(currentPassword, user.password);
      if (!isCurrentPasswordValid) {
        return new Response(
          JSON.stringify({ error: "Current password is incorrect" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      // Hash new password and update user
      const hashedNewPassword = await hashPassword(newPassword);
      await prisma.user.update({
        where: { id: user.id },
        data: { password: hashedNewPassword },
      });

      return new Response(
        JSON.stringify({ success: true, message: "Password changed successfully" }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    } catch (error) {
      console.error("Change password error:", error);
      return new Response(
        JSON.stringify({ error: "Internal server error" }),
        { status: 500, headers: { "Content-Type": "application/json" } },
      );
    }
  },
};
