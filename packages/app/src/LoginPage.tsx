import { createSignal, Show } from "solid-js";

import { logIn } from "@otelo/api";
import { LogoIcon } from "@otelo/icons";
import { Button, Callout } from "@otelo/ui";

interface LoginPageProps {
  readonly onLogin: () => void;
}

export default function LoginPage(props: LoginPageProps) {
  const [password, setPassword] = createSignal("");
  const [errorMessage, setErrorMessage] = createSignal<string>();
  const [checking, setChecking] = createSignal(false);

  const submitPassword = async (e: SubmitEvent) => {
    e.preventDefault();
    setChecking(true);
    setErrorMessage(undefined);
    try {
      await logIn(password());
      props.onLogin();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setChecking(false);
    }
  };

  return (
    <div class="flex h-dvh items-center justify-center bg-page px-4">
      <form
        class="flex w-full max-w-80 flex-col gap-3 rounded-lg border border-line bg-surface p-6"
        onSubmit={(e) => void submitPassword(e)}
      >
        <div class="mb-2 flex items-center gap-2 font-mono text-[17px] font-semibold text-ink">
          <LogoIcon size={24} />
          otelo
        </div>
        <label class="flex flex-col gap-1 text-muted">
          Password
          <input
            type="password"
            name="password"
            autocomplete="current-password"
            required
            autofocus
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
            class="h-8 rounded-md border border-line bg-surface px-2.5 font-mono text-ink focus:border-line-focus focus:outline-none"
          />
        </label>
        <Show when={errorMessage()}>
          {(message) => <Callout tone="error">{message()}</Callout>}
        </Show>
        <Button type="submit" variant="primary" disabled={checking() || password() === ""}>
          {checking() ? "Checking…" : "Log in"}
        </Button>
        <p class="text-xs text-muted">
          The password is the one that <code class="font-mono">otelo init</code> printed.
        </p>
      </form>
    </div>
  );
}
