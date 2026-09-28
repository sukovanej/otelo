import { Navigate, Route, Router } from "@solidjs/router";
import { render } from "solid-js/web";

import App from "./App";
import LogsPage from "./logs/LogsPage";
import ServicePage from "./services/ServicePage";
import ServicesPage from "./services/ServicesPage";
import TracePage from "./traces/TracePage";
import TracesPage from "./traces/TracesPage";

import "@siner/ui/fonts.css";
import "./app.css";

function NotFound() {
  return <div class="py-8 text-center text-muted">Nothing is at this address.</div>;
}

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

render(
  () => (
    <Router root={App}>
      <Route path="/" component={() => <Navigate href="/services" />} />
      <Route path="/services" component={ServicesPage} />
      <Route path="/services/:name" component={ServicePage} />
      <Route path="/logs" component={LogsPage} />
      <Route path="/traces" component={TracesPage} />
      <Route path="/traces/:id" component={TracePage} />
      <Route path="*" component={NotFound} />
    </Router>
  ),
  root,
);
