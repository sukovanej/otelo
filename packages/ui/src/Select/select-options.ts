export interface SelectOption<T extends string> {
  readonly value: T;
  readonly label: string;
  readonly section?: string;
}

interface SelectOptionGroup<T extends string> {
  readonly title: string;
  readonly options: ReadonlyArray<SelectOption<T>>;
}

export function groupSelectOptions<T extends string>(
  options: ReadonlyArray<SelectOption<T>>,
  search: string,
): SelectOptionGroup<T>[] {
  const words = search
    .toLowerCase()
    .split(/\s+/)
    .filter((word) => word !== "");
  const groupsByTitle = new Map<string, SelectOption<T>[]>();
  for (const option of options) {
    const text = `${option.label} ${option.value}`.toLowerCase();
    if (!words.every((word) => text.includes(word))) continue;
    const title = option.section ?? "";
    groupsByTitle.set(title, [...(groupsByTitle.get(title) ?? []), option]);
  }
  return [...groupsByTitle].map(([title, groupOptions]) => ({ title, options: groupOptions }));
}
