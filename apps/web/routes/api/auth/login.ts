import { Handlers } from "$fresh/server.ts";
import { getPrismaClient } from "../../../lib/auth/prisma-client.ts";
import { createSession, verifyPassword } from "../../../lib/auth/auth-utils.ts";
import { getConfig, parseExpirationTimeToSeconds } from "../../../lib/config.ts";

export const handler: Handlers = {
  async POST(req) {
    try {
      const body = await req.json();
      const { email, username, password } = body;

      // Validate input
      if ((!email && !username) || !password) {
        return new Response(
          JSON.stringify({ error: "Email/username and password are required" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      const prisma = await getPrismaClient();

      // Find user by email or username
      const user = await prisma.user.findFirst({
        where: {
          OR: [
            email ? { email: email.toLowerCase() } : {},
            username ? { username: username.toLowerCase() } : {},
          ].filter((condition) => Object.keys(condition).length > 0),
        },
      });

      if (!user) {
        return new Response(
          JSON.stringify({ error: "Invalid credentials" }),
          { status: 401, headers: { "Content-Type": "application/json" } },
        );
      }

      // Verify password
      const isValid = await verifyPassword(password, user.password);
      if (!isValid) {
        return new Response(
          JSON.stringify({ error: "Invalid credentials" }),
          { status: 401, headers: { "Content-Type": "application/json" } },
        );
      }

      // Create session
      const session = await createSession(user);
      const config = getConfig();
      const maxAge = parseExpirationTimeToSeconds(config.auth.jwtExpiresIn);

      // Build cookie string based on config
      const cookieOptions = [
        `${config.auth.sessionCookie.name}=${session.token}`,
        "Path=/",
        config.auth.sessionCookie.httpOnly ? "HttpOnly" : "",
        `SameSite=${config.auth.sessionCookie.sameSite}`,
        config.auth.sessionCookie.secure ? "Secure" : "",
        `Max-Age=${maxAge}`,
      ].filter(Boolean).join("; ");

      return new Response(
        JSON.stringify({
          success: true,
          user: session.user,
          token: session.token,
        }),
        {
          status: 200,
          headers: {
            "Content-Type": "application/json",
            "Set-Cookie": cookieOptions,
          },
        },
      );
    } catch (error) {
      console.error("Login error:", error);
      return new Response(
        JSON.stringify({ error: "Internal server error" }),
        { status: 500, headers: { "Content-Type": "application/json" } },
      );
    }
  },
};
