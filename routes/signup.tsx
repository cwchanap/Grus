import { PageProps } from "$fresh/server.ts";
import { Head } from "$fresh/runtime.ts";
import SignupForm from "../islands/SignupForm.tsx";

export default function SignupPage(_props: PageProps) {
  return (
    <>
      <Head>
        <title>Sign Up - Grus Drawing Game</title>
      </Head>
      <div class="min-h-screen bg-gradient-to-br from-purple-600 to-blue-600 flex items-center justify-center p-4">
        <SignupForm />
      </div>
    </>
  );
}
