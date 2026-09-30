let canvas: HTMLCanvasElement | undefined;

export function measureCaretX(input: HTMLInputElement, utf16Index: number): number {
  canvas ??= document.createElement("canvas");
  const context = canvas.getContext("2d");
  const style = getComputedStyle(input);
  const leftEdge = parseFloat(style.paddingLeft) + parseFloat(style.borderLeftWidth);
  if (!context) return leftEdge;
  context.font = style.font;
  const textWidth = context.measureText(input.value.slice(0, utf16Index)).width;
  return Math.max(0, Math.min(leftEdge + textWidth - input.scrollLeft, input.clientWidth));
}
