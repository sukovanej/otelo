import { createSignal } from "solid-js";

const [isLoginNeeded, setLoginNeeded] = createSignal(false);

export { isLoginNeeded };

export function askForLogin() {
  setLoginNeeded(true);
}

export function finishLogin() {
  setLoginNeeded(false);
}
