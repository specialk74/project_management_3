import { createSignal, onMount } from "solid-js";

interface Props {
  title: string;
  initial: string;
  onConfirm: (note: string) => void;
  onCancel: () => void;
}

export default function NoteDialog(props: Props) {
  const [value, setValue] = createSignal(props.initial);
  let textRef: HTMLTextAreaElement | undefined;

  onMount(() => textRef?.focus());

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") props.onCancel();
    if (e.key === "Enter" && e.ctrlKey) props.onConfirm(value());
  }

  return (
    <div class="dialog-overlay" onClick={props.onCancel}>
      <div class="dialog-box" onClick={(e) => e.stopPropagation()}>
        <div class="dialog-title">{props.title}</div>
        <textarea
          ref={textRef}
          value={value()}
          onInput={(e) => setValue(e.currentTarget.value)}
          onKeyDown={handleKey}
          placeholder="Nota..."
          rows={4}
        />
        <div class="dialog-buttons">
          <button onClick={() => props.onConfirm(value())}>OK</button>
          <button onClick={props.onCancel}>Annulla</button>
        </div>
      </div>
    </div>
  );
}
