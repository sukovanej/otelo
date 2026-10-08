import { type Accessor, createContext, createSignal } from "solid-js";

export interface LoginState {
  readonly isLoginNeeded: Accessor<boolean>;
  readonly askForLogin: () => void;
  readonly finishLogin: () => void;
}

export const LoginContext = createContext<LoginState>();

export function createLoginState(): LoginState {
  const [isLoginNeeded, setLoginNeeded] = createSignal(false);
  return {
    isLoginNeeded,
    askForLogin: () => setLoginNeeded(true),
    finishLogin: () => setLoginNeeded(false),
  };
}
