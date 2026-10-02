import type { JSX } from "@solidjs/web";
import { Errored } from "solid-js";

import { Callout } from "@otelo/ui";

interface FetchErrorBoundaryProps {
  readonly children: JSX.Element;
}

export default function FetchErrorBoundary(props: FetchErrorBoundaryProps) {
  return (
    <Errored fallback={(error) => <Callout tone="error">{describeError(error())}</Callout>}>
      {props.children}
    </Errored>
  );
}

function describeError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
