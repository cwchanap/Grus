import { Handlers } from "$fresh/server.ts";
import { getSession } from "../../../lib/auth/auth-utils.ts";
import { getConfig } from "../../../lib/config.ts";
import { getPrismaClient } from "../../../lib/auth/prisma-client.ts";

export const handler: Handlers = {
  async POST(req) {
    try {
      const body = await req.json().catch(() => ({}));
      const { avatar } = body as { avatar?: string };

      // Validate input
      if (typeof avatar !== "string") {
        return new Response(
          JSON.stringify({ error: "Avatar must be a base64 data URL string" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      // Accept empty string to clear avatar
      if (avatar.length > 0) {
        // Expect a data URL like: data:image/png;base64,....
        const isDataUrl = avatar.startsWith("data:image/") && avatar.includes(";base64,");
        if (!isDataUrl) {
          return new Response(
            JSON.stringify({ error: "Invalid avatar format. Expected base64 image data URL." }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }
        // Optional size check: disallow very large payloads (>500KB)
        if (avatar.length > 500 * 1024) {
          return new Response(
            JSON.stringify({ error: "Avatar is too large. Please upload a smaller image." }),
            { status: 413, headers: { "Content-Type": "application/json" } },
          );
        }
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

      const updated = await prisma.user.update({
        where: { id: session.user.id },
        data: { avatar: avatar.trim() || null },
        select: { id: true, email: true, username: true, name: true, avatar: true },
      });

      return new Response(
        JSON.stringify({ success: true, user: updated }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    } catch (error) {
      console.error("Update avatar error:", error);
      return new Response(
        JSON.stringify({ error: "Internal server error" }),
        { status: 500, headers: { "Content-Type": "application/json" } },
      );
    }
  },
};
