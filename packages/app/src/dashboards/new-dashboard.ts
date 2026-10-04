// The router state of the page a new dashboard opens on, which selects its name
// so that typing renames it.
export const NEW_DASHBOARD_STATE: NewDashboardState = { isNew: true };

interface NewDashboardState {
  readonly isNew: true;
}

export function isNewDashboardState(state: unknown): boolean {
  return typeof state === "object" && state !== null && "isNew" in state && state.isNew === true;
}
