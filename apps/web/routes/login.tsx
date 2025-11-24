import { PageProps } from "$fresh/server.ts";
import { Head } from "$fresh/runtime.ts";
import LoginForm from "../islands/LoginForm.tsx";

export default function LoginPage(_props: PageProps) {
  return (
    <>
      <Head>
        <title>Login - Grus Drawing Game</title>
      </Head>
      <div class="min-h-screen bg-gradient-to-br from-purple-600 to-blue-600 flex items-center justify-center p-4">
        <LoginForm />
      </div>
    </>
  );
}
