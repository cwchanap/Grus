import { Handlers } from "$fresh/server.ts";
import { getSession } from "../../../lib/auth/auth-utils.ts";
import { getConfig } from "../../../lib/config.ts";

export const handler: Handlers = {
  async GET(req) {
    try {
      const config = getConfig();
      
      // Get token from cookie or Authorization header
      const cookie = req.headers.get("Cookie");
      const authHeader = req.headers.get("Authorization");

      const cookieName = config.auth.sessionCookie.name;
      let token = cookie?.split(";")
        .find((c) => c.trim().startsWith(`${cookieName}=`))
        ?.split("=")[1];

      // If no cookie token, check Authorization header
      if (!token && authHeader?.startsWith("Bearer ")) {
        token = authHeader.substring(7);
      }

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

      return new Response(
        JSON.stringify({
          success: true,
          user: session.user,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    } catch (error) {
      console.error("Get user error:", error);
      return new Response(
        JSON.stringify({ error: "Internal server error" }),
        { status: 500, headers: { "Content-Type": "application/json" } },
      );
    }
  },
};
