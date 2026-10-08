import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import {
  toDashboardList,
  toEmptyMetricSeries,
  toLogGroup,
  toLogGroups,
  toMetricList,
  toRouteGroup,
  toService,
  toServices,
  toSpanGroups,
  toSpans,
  toTrace,
  toTraces,
  toTraceSpan,
} from "./fixtures";
import { mountApp } from "./mount";

const ORDERS_ROUTE = toRouteGroup("GET", "/orders");

const CHECKOUT_TRACE = toTrace([
  toTraceSpan("root", null, 0, 100),
  toTraceSpan("charge", "root", 10, 60),
]);

test("the services page lists each service", async () => {
  const api = createFakeApi({ getServices: () => Promise.resolve(toServices(["api", "worker"])) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/services");
    await expect.element(page.getByRole("row", { name: /worker/ })).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("a service page opens the spans of one of its routes", async () => {
  const api = createFakeApi({
    getService: (name) => Promise.resolve(toService(name)),
    getSpanGroups: (query) =>
      Promise.resolve(toSpanGroups(query.by?.includes("http.route") ? [ORDERS_ROUTE] : [])),
    getTraces: () => Promise.resolve(toTraces([])),
    getLogGroups: () => Promise.resolve(toLogGroups([])),
    getMetricSeries: (name) => Promise.resolve(toEmptyMetricSeries(name)),
    getSpans: () => Promise.resolve(toSpans(CHECKOUT_TRACE.spans)),
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/services/api");
    await userEvent.click(page.getByRole("row", { name: /\/orders/ }).first());
    await expect.element(page.getByRole("dialog")).toBeInTheDocument();
    await expect.element(page.getByRole("dialog").getByText("step charge")).toBeInTheDocument();
    await expect.element(page.getByRole("dialog").getByText("Loading…")).not.toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("the logs page groups its lines by template", async () => {
  const api = createFakeApi({
    getLogGroups: () => Promise.resolve(toLogGroups([toLogGroup("payment <num> declined")])),
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/logs?view=groups");
    await expect.element(page.getByText("<num>")).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("the spans of the traces page open their trace", async () => {
  const api = createFakeApi({
    getSpans: () => Promise.resolve(toSpans(CHECKOUT_TRACE.spans)),
    getTrace: () => Promise.resolve(CHECKOUT_TRACE),
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/traces?view=spans");
    await userEvent.click(page.getByRole("row", { name: /step charge/ }));
    await userEvent.click(page.getByRole("link", { name: "Trace", exact: true }));
    await expect.element(page.getByRole("dialog")).toBeInTheDocument();
    await expect
      .element(page.getByRole("dialog").getByRole("row", { name: /step root/ }))
      .toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("the metrics page draws the metric picked from its list", async () => {
  const api = createFakeApi({
    getMetrics: () => Promise.resolve(toMetricList(["process.memory.usage"])),
    getMetricSeries: (name) => Promise.resolve(toEmptyMetricSeries(name)),
  });

  const { artifact } = await captureArtifact(async () => {
    const { history } = mountApp(api, "/metrics");
    await userEvent.click(page.getByRole("link", { name: /process\.memory\.usage/ }));
    await expect.poll(() => history.get()).toContain("/metrics/process.memory.usage");
    await expect
      .element(page.getByRole("heading", { name: "process.memory.usage" }))
      .toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("the dashboards page lists the dashboards", async () => {
  const api = createFakeApi({
    listDashboards: () => Promise.resolve(toDashboardList(["Latency", "Errors"])),
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/dashboards");
    await expect.element(page.getByRole("row", { name: /Errors/ })).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("a login with the password brings the app back", async () => {
  const sentPasswords: string[] = [];
  const api = createFakeApi({
    logIn: (password) => {
      sentPasswords.push(password);
      return Promise.resolve();
    },
  });

  const { artifact } = await captureArtifact(async () => {
    const { login } = mountApp(api, "/nowhere");
    login.askForLogin();
    await userEvent.fill(page.getByLabelText("Password"), "hunter2");
    await userEvent.click(page.getByRole("button", { name: "Log in" }));
    await expect.element(page.getByRole("button", { name: "Log out" })).toBeInTheDocument();
  });

  expect(sentPasswords).toEqual(["hunter2"]);
  expectNoReactivityMistakes(artifact);
});
