import { Handlers } from "$fresh/server.ts";
import { createSession, hashPassword } from "../../../lib/auth/auth-utils.ts";
import { getPrismaClient } from "../../../lib/auth/prisma-client.ts";

export const handler: Handlers = {
  async POST(req) {
    try {
      const body = await req.json();
      const { email, username, password, name } = body;

      // Validate input
      if (!email || !username || !password) {
        return new Response(
          JSON.stringify({ error: "Email, username, and password are required" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      // Validate email format
      const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
      if (!emailRegex.test(email)) {
        return new Response(
          JSON.stringify({ error: "Invalid email format" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      // Validate password length
      if (password.length < 6) {
        return new Response(
          JSON.stringify({ error: "Password must be at least 6 characters" }),
          { status: 400, headers: { "Content-Type": "application/json" } },
        );
      }

      const prisma = await getPrismaClient();

      // Check if user already exists
      const existingUser = await prisma.user.findFirst({
        where: {
          OR: [
            { email: email.toLowerCase() },
            { username: username.toLowerCase() },
          ],
        },
      });

      if (existingUser) {
        return new Response(
          JSON.stringify({ error: "User with this email or username already exists" }),
          { status: 409, headers: { "Content-Type": "application/json" } },
        );
      }

      // Create new user
      const hashedPassword = await hashPassword(password);
      const newUser = await prisma.user.create({
        data: {
          email: email.toLowerCase(),
          username: username.toLowerCase(),
          password: hashedPassword,
          name: name || null,
        },
      });

      // Create session
      const session = await createSession(newUser);

      return new Response(
        JSON.stringify({
          success: true,
          user: session.user,
          token: session.token,
        }),
        {
          status: 201,
          headers: {
            "Content-Type": "application/json",
            "Set-Cookie": `${
              Deno.env.get("SESSION_COOKIE_NAME") || "grus_session"
            }=${session.token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=${7 * 24 * 60 * 60}`,
          },
        },
      );
    } catch (error) {
      console.error("Signup error:", error);
      return new Response(
        JSON.stringify({ error: "Internal server error" }),
        { status: 500, headers: { "Content-Type": "application/json" } },
      );
    }
  },
};
