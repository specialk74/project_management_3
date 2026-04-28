import { createSignal } from "solid-js";
import type { AppStateDto } from "../types";

interface Props {
  state: AppStateDto;
  onSave: () => void;
  onSaveAs: () => void;
  onOpen: () => void;
  onNewProject: () => void;
  onAddWorker: (name: string) => void;
  onAddDev: (name: string) => void;
  onSearch: (name: string) => void;
}

export default function Toolbar(props: Props) {
  const [workerInput, setWorkerInput] = createSignal("");
  const [devInput, setDevInput] = createSignal("");
  const [searchInput, setSearchInput] = createSignal("");

  function submitWorker() {
    const v = workerInput().trim();
    if (v) {
      props.onAddWorker(v);
      setWorkerInput("");
    }
  }

  function submitDev() {
    const v = devInput().trim();
    if (v) {
      props.onAddDev(v);
      setDevInput("");
    }
  }

  function handleSearch(val: string) {
    setSearchInput(val);
    props.onSearch(val);
  }

  const fileName = () => {
    const f = props.state.current_file;
    return f.split("/").pop() ?? f;
  };

  return (
    <div class="toolbar">
      <button onClick={props.onOpen}>Apri</button>
      <button onClick={props.onSave}>Salva</button>
      <button onClick={props.onSaveAs}>Salva come...</button>
      <span class="sep" />
      <button onClick={props.onNewProject}>+ Progetto</button>
      <span class="sep" />
      <input
        placeholder="Nuovo lavoratore..."
        value={workerInput()}
        onInput={(e) => setWorkerInput(e.currentTarget.value)}
        onKeyDown={(e) => e.key === "Enter" && submitWorker()}
      />
      <button onClick={submitWorker}>+ Lavoratore</button>
      <span class="sep" />
      <input
        placeholder="Nuova funzione..."
        value={devInput()}
        onInput={(e) => setDevInput(e.currentTarget.value)}
        onKeyDown={(e) => e.key === "Enter" && submitDev()}
      />
      <button onClick={submitDev}>+ Funzione</button>
      <span class="sep" />
      <input
        placeholder="Cerca lavoratore..."
        value={searchInput()}
        onInput={(e) => handleSearch(e.currentTarget.value)}
      />
      <span class="sep" />
      <span class="file-info">
        {props.state.changed && <span class="changed-dot">● </span>}
        {fileName()}
      </span>
    </div>
  );
}
