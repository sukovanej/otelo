import { createRouter, useNavigate } from "@solidjs/router";
import { render } from "@solidjs/web";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";

import App from "./App";
import LogsPage from "./logs/LogsPage";
import MetricsPage from "./metrics/MetricsPage";
import ServicePage from "./services/ServicePage";
import ServicesPage from "./services/ServicesPage";
import TracePage from "./traces/TracePage";
import TracesPage from "./traces/TracesPage";

import "@otelo/ui/fonts.css";
import "./app.css";

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

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
      <Router>{(props) => <App {...props} />}</Router>
    </QueryClientProvider>
  ),
  root,
);
