import { createSignal, onMount } from "solid-js";

interface Props {
  title: string;
  initial: number;
  onConfirm: (hours: number) => void;
  onCancel: () => void;
}

export default function HoursDialog(props: Props) {
  const [value, setValue] = createSignal(String(props.initial));
  let inputRef: HTMLInputElement | undefined;

  onMount(() => {
    inputRef?.focus();
    inputRef?.select();
  });

  function commit() {
    const h = parseInt(value(), 10);
    if (!isNaN(h) && h >= 1 && h <= 40) {
      props.onConfirm(h);
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Enter") commit();
    if (e.key === "Escape") props.onCancel();
  }

  return (
    <div class="dialog-overlay" onClick={props.onCancel}>
      <div class="dialog-box" onClick={(e) => e.stopPropagation()}>
        <div class="dialog-title">{props.title}</div>
        <input
          ref={inputRef}
          type="number"
          min="1"
          max="40"
          value={value()}
          onInput={(e) => setValue(e.currentTarget.value)}
          onKeyDown={handleKey}
        />
        <div class="dialog-buttons">
          <button onClick={commit}>OK</button>
          <button onClick={props.onCancel}>Annulla</button>
        </div>
      </div>
    </div>
  );
}
