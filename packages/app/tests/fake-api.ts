import type { Api } from "@otelo/api";

// Each endpoint a test leaves out fails the request that reaches it, with its name.
export function createFakeApi(endpoints: Partial<Api>): Api {
  return {
    getLogs: endpoints.getLogs ?? rejectUnexpectedCall("getLogs"),
    getLogGroups: endpoints.getLogGroups ?? rejectUnexpectedCall("getLogGroups"),
    getLogCounts: endpoints.getLogCounts ?? rejectUnexpectedCall("getLogCounts"),
    getSpans: endpoints.getSpans ?? rejectUnexpectedCall("getSpans"),
    getSpanGroups: endpoints.getSpanGroups ?? rejectUnexpectedCall("getSpanGroups"),
    getTraces: endpoints.getTraces ?? rejectUnexpectedCall("getTraces"),
    getTrace: endpoints.getTrace ?? rejectUnexpectedCall("getTrace"),
    getMetrics: endpoints.getMetrics ?? rejectUnexpectedCall("getMetrics"),
    getMetricSeries: endpoints.getMetricSeries ?? rejectUnexpectedCall("getMetricSeries"),
    getServices: endpoints.getServices ?? rejectUnexpectedCall("getServices"),
    getService: endpoints.getService ?? rejectUnexpectedCall("getService"),
    getAttributeKeys: endpoints.getAttributeKeys ?? rejectUnexpectedCall("getAttributeKeys"),
    completeQuery: endpoints.completeQuery ?? rejectUnexpectedCall("completeQuery"),
    listDashboards: endpoints.listDashboards ?? rejectUnexpectedCall("listDashboards"),
    getDashboard: endpoints.getDashboard ?? rejectUnexpectedCall("getDashboard"),
    createDashboard: endpoints.createDashboard ?? rejectUnexpectedCall("createDashboard"),
    replaceDashboard: endpoints.replaceDashboard ?? rejectUnexpectedCall("replaceDashboard"),
    deleteDashboard: endpoints.deleteDashboard ?? rejectUnexpectedCall("deleteDashboard"),
    addIndex: endpoints.addIndex ?? rejectUnexpectedCall("addIndex"),
    logIn: endpoints.logIn ?? rejectUnexpectedCall("logIn"),
    logOut: endpoints.logOut ?? rejectUnexpectedCall("logOut"),
  };
}

function rejectUnexpectedCall(endpoint: keyof Api): () => Promise<never> {
  return () => Promise.reject(new Error(`The test did not expect a call to ${endpoint}`));
}
