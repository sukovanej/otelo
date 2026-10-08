import { memoryHistory, type MemoryHistoryAdapter } from "@solidjs/router";
import type { JSX } from "@solidjs/web";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { onTestFinished } from "vitest";

import type { Api } from "@otelo/api";
import { mountView } from "@otelo/testing";

import { ApiContext } from "../src/api";
import AppRoot from "../src/AppRoot";
import { createLoginState, type LoginState } from "../src/login";

export interface MountedView {
  readonly container: HTMLElement;
  readonly queryClient: QueryClient;
}

export interface MountedApp extends MountedView {
  readonly login: LoginState;
  readonly history: MemoryHistoryAdapter;
}

export function mountApp(api: Api, path: string): MountedApp {
  const queryClient = createQueryClient();
  const login = createLoginState();
  const history = memoryHistory(path);
  const container = mountView(() => (
    <AppRoot api={api} login={login} queryClient={queryClient} history={history} />
  ));
  return { container, queryClient, login, history };
}

export function mountWithApi(api: Api, view: () => JSX.Element): MountedView {
  const queryClient = createQueryClient();
  const container = mountView(() => (
    <ApiContext value={api}>
      <QueryClientProvider client={queryClient}>{view()}</QueryClientProvider>
    </ApiContext>
  ));
  return { container, queryClient };
}

function createQueryClient(): QueryClient {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  onTestFinished(() => queryClient.clear());
  return queryClient;
}
