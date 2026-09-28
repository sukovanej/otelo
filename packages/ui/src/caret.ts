let canvas: HTMLCanvasElement | undefined;

/**
 * The distance in pixels from the left edge of `input` to the point before
 * the UTF-16 index `index` of its value, as it shows now: with its padding,
 * its border, and its scroll.
 */
export function textX(input: HTMLInputElement, index: number): number {
  canvas ??= document.createElement("canvas");
  const context = canvas.getContext("2d");
  const style = getComputedStyle(input);
  const edge = parseFloat(style.paddingLeft) + parseFloat(style.borderLeftWidth);
  if (!context) return edge;
  context.font = style.font;
  const width = context.measureText(input.value.slice(0, index)).width;
  return Math.max(0, Math.min(edge + width - input.scrollLeft, input.clientWidth));
}
