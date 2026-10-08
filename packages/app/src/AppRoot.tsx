import { createRouter, type RouterHistory, useNavigate } from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Show, untrack } from "solid-js";

import type { Api } from "@otelo/api";

import { ApiContext } from "./api";
import App from "./App";
import DashboardPage from "./dashboards/DashboardPage";
import DashboardsPage from "./dashboards/DashboardsPage";
import { LoginContext, type LoginState } from "./login";
import LoginPage from "./LoginPage";
import LogsPage from "./logs/LogsPage";
import MetricsPage from "./metrics/MetricsPage";
import ServicePage from "./services/ServicePage";
import ServicesPage from "./services/ServicesPage";
import TracePage from "./traces/TracePage";
import TracesPage from "./traces/TracesPage";

const ROUTES = [
  { path: "/", component: RedirectToServices },
  { path: "/services", component: ServicesPage },
  { path: "/services/:name", component: ServicePage },
  { path: "/logs", component: LogsPage },
  { path: "/traces", component: TracesPage },
  { path: "/traces/:id", component: TracePage },
  { path: "/metrics/:name?", component: MetricsPage },
  { path: "/dashboards", component: DashboardsPage },
  { path: "/dashboards/:id", component: DashboardPage },
  { path: "*", component: NotFound },
];

interface AppRootProps {
  readonly api: Api;
  readonly login: LoginState;
  readonly queryClient: QueryClient;
  readonly history: RouterHistory;
}

export default function AppRoot(props: AppRootProps) {
  const Router = createRouter({
    routes: ROUTES,
    history: untrack(() => props.history),
    preloadLinks: false,
  });
  return (
    <ApiContext value={props.api}>
      <LoginContext value={props.login}>
        <QueryClientProvider client={props.queryClient}>
          <Show
            when={!props.login.isLoginNeeded()}
            fallback={
              <LoginPage
                onLogin={() => {
                  props.queryClient.clear();
                  props.login.finishLogin();
                }}
              />
            }
          >
            <Router>{(routeProps) => <App {...routeProps} />}</Router>
          </Show>
        </QueryClientProvider>
      </LoginContext>
    </ApiContext>
  );
}

function RedirectToServices() {
  const navigate = useNavigate();
  navigate("/services", { replace: true });
  return null;
}

function NotFound() {
  return <div class="py-8 text-center text-muted">Nothing is at this address.</div>;
}
