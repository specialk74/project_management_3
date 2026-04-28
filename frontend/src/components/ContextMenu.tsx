import { For, onCleanup, onMount } from "solid-js";

interface Item {
  label: string;
  action: () => void;
}

interface Props {
  x: number;
  y: number;
  items: Item[];
  onClose: () => void;
}

export default function ContextMenu(props: Props) {
  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") props.onClose();
  }

  onMount(() => document.addEventListener("keydown", handleKey));
  onCleanup(() => document.removeEventListener("keydown", handleKey));

  return (
    <div
      class="ctx-menu"
      style={{ left: `${props.x}px`, top: `${props.y}px` }}
      onClick={(e) => e.stopPropagation()}
    >
      <For each={props.items}>
        {(item) => (
          <div
            class="ctx-menu-item"
            onClick={() => item.action()}
          >
            {item.label}
          </div>
        )}
      </For>
    </div>
  );
}
