import { createRouter, useNavigate } from "@solidjs/router";
import { render } from "@solidjs/web";
import { QueryCache, QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Show } from "solid-js";

import { isUnauthorizedError } from "@otelo/api";

import App from "./App";
import { askForLogin, finishLogin, isLoginNeeded } from "./login";
import LoginPage from "./LoginPage";
import LogsPage from "./logs/LogsPage";
import MetricsPage from "./metrics/MetricsPage";
import ServicePage from "./services/ServicePage";
import ServicesPage from "./services/ServicesPage";
import TracePage from "./traces/TracePage";
import TracesPage from "./traces/TracesPage";

import "@otelo/ui/fonts.css";
import "./app.css";

const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error) => {
      if (isUnauthorizedError(error)) askForLogin();
    },
  }),
  defaultOptions: { queries: { retry: false } },
});

const Router = createRouter({
  routes: [
    { path: "/", component: RedirectToServices },
    { path: "/services", component: ServicesPage },
    { path: "/services/:name", component: ServicePage },
    { path: "/logs", component: LogsPage },
    { path: "/traces", component: TracesPage },
    { path: "/traces/:id", component: TracePage },
    { path: "/metrics/:name?", component: MetricsPage },
    { path: "*", component: NotFound },
  ],
});

function RedirectToServices() {
  const navigate = useNavigate();
  navigate("/services", { replace: true });
  return null;
}

function NotFound() {
  return <div class="py-8 text-center text-muted">Nothing is at this address.</div>;
}

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

render(
  () => (
    <QueryClientProvider client={queryClient}>
      <Show
        when={!isLoginNeeded()}
        fallback={
          <LoginPage
            onLogin={() => {
              queryClient.clear();
              finishLogin();
            }}
          />
        }
      >
        <Router>{(props) => <App {...props} />}</Router>
      </Show>
    </QueryClientProvider>
  ),
  root,
);
