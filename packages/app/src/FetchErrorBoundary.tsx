import type { JSX } from "@solidjs/web";
import { Errored } from "solid-js";

import { Callout } from "@otelo/ui";

import { describeError } from "./fetch";

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
