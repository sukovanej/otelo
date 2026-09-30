export const NO_ITEM = -1;

interface KeyPress {
  readonly key: string;
  readonly ctrlKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
}

type ListStep = -1 | 0 | 1;

export function toListStep(keyPress: KeyPress): ListStep {
  if (keyPress.key === "ArrowDown" || isCtrlLetter(keyPress, "j")) return 1;
  if (keyPress.key === "ArrowUp" || isCtrlLetter(keyPress, "k")) return -1;
  return 0;
}

export function moveListIndex(
  index: number,
  step: number,
  count: number,
  hasNoItemStop = false,
): number {
  const lowestIndex = hasNoItemStop ? NO_ITEM : 0;
  const stopCount = count - lowestIndex;
  if (stopCount <= 0) return lowestIndex;
  return ((((index - lowestIndex + step) % stopCount) + stopCount) % stopCount) + lowestIndex;
}

function isCtrlLetter(keyPress: KeyPress, letter: string): boolean {
  return (
    keyPress.ctrlKey &&
    !keyPress.altKey &&
    !keyPress.metaKey &&
    !keyPress.shiftKey &&
    keyPress.key.toLowerCase() === letter
  );
}
