import { Handlers, PageProps } from "$fresh/server.ts";
import { Head } from "$fresh/runtime.ts";
import { getSession } from "../lib/auth/auth-utils.ts";
import { getConfig } from "../lib/config.ts";
import UserProfile from "../islands/UserProfile.tsx";
import type { UserPayload } from "../lib/auth/auth-utils.ts";

interface Data {
  user: UserPayload | null;
}

export const handler: Handlers<Data> = {
  async GET(req, ctx) {
    // Check if user is authenticated
    const config = getConfig();
    const cookie = req.headers.get("Cookie");
    const cookieName = config.auth.sessionCookie.name;
    const token = cookie?.split(";")
      .find((c) => c.trim().startsWith(`${cookieName}=`))
      ?.split("=")[1];

    let user = null;
    if (token) {
      const session = await getSession(token);
      user = session?.user || null;
    }

    // Redirect to login if not authenticated
    if (!user) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: "/login",
        },
      });
    }

    return ctx.render({ user });
  },
};

export default function ProfilePage({ data }: PageProps<Data>) {
  const { user } = data;

  return (
    <>
      <Head>
        <title>Profile - Grus Drawing Game</title>
      </Head>
      <div class="min-h-screen bg-gradient-to-br from-purple-600 to-blue-600 p-4">
        <UserProfile user={user!} />
      </div>
    </>
  );
}
