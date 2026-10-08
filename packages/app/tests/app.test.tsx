import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { createDeferred, expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { mountApp } from "./mount";

test("log out shows that it is logging out, then asks for the password", async () => {
  const loggedOut = createDeferred<void>();
  const api = createFakeApi({ logOut: () => loggedOut.promise });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/nowhere");
    await userEvent.click(page.getByRole("button", { name: "Log out" }));
    await expect.element(page.getByRole("button", { name: "Logging out…" })).toBeDisabled();

    loggedOut.resolve();
    await expect.element(page.getByLabelText("Password")).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("a log out the daemon refuses says why and stays logged in", async () => {
  const api = createFakeApi({ logOut: () => Promise.reject(new Error("503 Service Unavailable")) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/nowhere");
    await userEvent.click(page.getByRole("button", { name: "Log out" }));

    await expect.element(page.getByRole("alert")).toHaveTextContent("503 Service Unavailable");
    await expect.element(page.getByRole("button", { name: "Log out" })).toBeEnabled();
    await expect.element(page.getByLabelText("Password")).not.toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});
