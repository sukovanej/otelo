import { browserHistory } from "@solidjs/router";
import { render } from "@solidjs/web";
import { QueryCache, QueryClient } from "@tanstack/solid-query";

import { createApi, isUnauthorizedError } from "@otelo/api";

import AppRoot from "./AppRoot";
import { trimUnwatchedListsToFirstPage } from "./fetch";
import { createLoginState } from "./login";

import "@otelo/ui/fonts.css";
import "./app.css";

const api = createApi(fetch);
const login = createLoginState();
const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error) => {
      if (isUnauthorizedError(error)) login.askForLogin();
    },
  }),
  defaultOptions: { queries: { retry: false } },
});
trimUnwatchedListsToFirstPage(queryClient);

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

render(
  () => <AppRoot api={api} login={login} queryClient={queryClient} history={browserHistory()} />,
  root,
);
