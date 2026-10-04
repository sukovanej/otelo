import { type Params, useParams } from "@solidjs/router";
import { Show } from "solid-js";

import DashboardPageView from "./dashboard-page-view";

interface DashboardPathParams extends Params {
  readonly id: string;
}

export default function DashboardPage() {
  const params = useParams<DashboardPathParams>();
  return (
    <Show when={params.id} keyed>
      {(id) => <DashboardPageView id={Number(id)} />}
    </Show>
  );
}
