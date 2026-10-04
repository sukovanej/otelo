interface MeterProps {
  readonly share: number;
}

export default function Meter(props: MeterProps) {
  return (
    <span class="block h-1.5 overflow-hidden rounded-full bg-hover" aria-hidden="true">
      <span
        class="block h-full min-w-px rounded-full bg-accent/45 dark:bg-accent/75"
        style={{ width: `${Math.max(0, Math.min(1, props.share)) * 100}%` }}
      />
    </span>
  );
}
