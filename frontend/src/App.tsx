import { createSignal, onMount, Show } from "solid-js";
import type { AppStateDto } from "./types";
import * as api from "./api";
import Toolbar from "./components/Toolbar";
import Grid from "./components/Grid";
import "./app.css";

export default function App() {
  const [state, setState] = createSignal<AppStateDto | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  onMount(async () => {
    try {
      const s = await api.getState();
      setState(s);
    } catch (e) {
      setError(String(e));
    }
  });

  async function handleSave() {
    const s = state();
    if (!s) return;
    try {
      let path = s.current_file;
      if (!path || path === "workers.ron") {
        const picked = await api.pickSaveFile(path);
        if (!picked) return;
        path = picked;
      }
      await api.saveFile(path);
      setState((prev) => prev ? { ...prev, changed: false, current_file: path } : prev);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleSaveAs() {
    const s = state();
    if (!s) return;
    try {
      const picked = await api.pickSaveFile(s.current_file);
      if (!picked) return;
      await api.saveFile(picked);
      setState((prev) => prev ? { ...prev, changed: false, current_file: picked } : prev);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleOpen() {
    try {
      const picked = await api.pickOpenFile();
      if (!picked) return;
      const newState = await api.openFile(picked);
      setState(newState);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleNewProject() {
    try {
      setState(await api.newProject());
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleAddWorker(name: string) {
    try {
      setState(await api.addWorker(name));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleAddDev(name: string) {
    try {
      setState(await api.addDev(name));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleSearch(name: string) {
    try {
      setState(await api.searchWorker(name));
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div class="app">
      <Show when={error()}>
        <div class="error-banner">{error()}</div>
      </Show>
      <Show when={state()} fallback={<div class="loading">Caricamento...</div>}>
        {(s) => (
          <>
            <Toolbar
              state={s()}
              onSave={handleSave}
              onSaveAs={handleSaveAs}
              onOpen={handleOpen}
              onNewProject={handleNewProject}
              onAddWorker={handleAddWorker}
              onAddDev={handleAddDev}
              onSearch={handleSearch}
            />
            <Grid state={s()} onStateChange={setState} />
          </>
        )}
      </Show>
    </div>
  );
}
