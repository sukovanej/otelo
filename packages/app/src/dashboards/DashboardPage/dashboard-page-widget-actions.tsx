import { CopyIcon, PencilIcon, TrashIcon } from "@otelo/icons";
import { ActionMenu, type ActionMenuItem, Button } from "@otelo/ui";

interface DashboardPageWidgetActionsProps {
  readonly onEdit: () => void;
  readonly onDuplicate: () => void;
  readonly onRemove: () => void;
}

export default function DashboardPageWidgetActions(props: DashboardPageWidgetActionsProps) {
  const menuItems: ReadonlyArray<ActionMenuItem> = [
    {
      label: "Edit",
      icon: () => <PencilIcon size={14} />,
      tone: "default",
      onSelect: () => props.onEdit(),
    },
    {
      label: "Duplicate",
      icon: () => <CopyIcon size={14} />,
      tone: "default",
      onSelect: () => props.onDuplicate(),
    },
    {
      label: "Remove",
      icon: () => <TrashIcon size={14} />,
      tone: "danger",
      onSelect: () => props.onRemove(),
    },
  ];
  return (
    <div class="opacity-0 transition-opacity group-hover/widget:opacity-100 focus-within:opacity-100 pointer-coarse:opacity-100">
      <div class="hidden items-center gap-0.5 @[16rem]/widget:flex">
        <Button
          size="sm"
          variant="ghost"
          aria-label="Edit the widget"
          title="Edit"
          onClick={() => props.onEdit()}
        >
          <PencilIcon size={14} />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          aria-label="Duplicate the widget"
          title="Duplicate"
          onClick={() => props.onDuplicate()}
        >
          <CopyIcon size={14} />
        </Button>
        <Button
          size="sm"
          variant="ghost-danger"
          aria-label="Remove the widget"
          title="Remove"
          onClick={() => props.onRemove()}
        >
          <TrashIcon size={14} />
        </Button>
      </div>
      <div class="@[16rem]/widget:hidden">
        <ActionMenu label="Widget actions" items={menuItems} />
      </div>
    </div>
  );
}
