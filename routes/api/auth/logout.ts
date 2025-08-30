import { Handlers } from "$fresh/server.ts";
import { deleteSession } from "../../../lib/auth/auth-utils.ts";
import { getConfig } from "../../../lib/config.ts";

export const handler: Handlers = {
  async POST(req) {
    try {
      const config = getConfig();
      
      // Get token from cookie
      const cookie = req.headers.get("Cookie");
      const cookieName = config.auth.sessionCookie.name;
      const token = cookie?.split(";")
        .find((c) => c.trim().startsWith(`${cookieName}=`))
        ?.split("=")[1];

      if (token) {
        await deleteSession(token);
      }

      return new Response(
        JSON.stringify({ success: true }),
        {
          status: 200,
          headers: {
            "Content-Type": "application/json",
            "Set-Cookie": `${cookieName}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0`,
          },
        },
      );
    } catch (error) {
      console.error("Logout error:", error);
      return new Response(
        JSON.stringify({ error: "Internal server error" }),
        { status: 500, headers: { "Content-Type": "application/json" } },
      );
    }
  },
};
