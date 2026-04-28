import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { AppStateDto, CellUpdate } from "./types";

export const getState = () => invoke<AppStateDto>("get_state");

export const setCells = (
  project_idx: number,
  dev_id: number,
  week: number,
  cells: CellUpdate[]
) => invoke<AppStateDto>("set_cells", { project_idx, dev_id, week, cells });

export const setDevEffort = (project_idx: number, dev_id: number, effort_pct: number) =>
  invoke<AppStateDto>("set_dev_effort", { project_idx, dev_id, effort_pct });

export const setNote = (
  project_idx: number,
  dev_id: number,
  week: number,
  worker_name: string,
  note: string
) => invoke<AppStateDto>("set_note", { project_idx, dev_id, week, worker_name, note });

export const setDevNote = (project_idx: number, dev_id: number, note: string) =>
  invoke<AppStateDto>("set_dev_note", { project_idx, dev_id, note });

export const addRow = (project_id: number, dev_id: number) =>
  invoke<AppStateDto>("add_row", { project_id, dev_id });

export const delRow = (project_id: number, dev_id: number) =>
  invoke<AppStateDto>("del_row", { project_id, dev_id });

export const newProject = () => invoke<AppStateDto>("new_project");

export const renameProject = (project_idx: number, name: string) =>
  invoke<AppStateDto>("rename_project", { project_idx, name });

export const addDevToProject = (project_idx: number, dev_id: number, add: boolean) =>
  invoke<AppStateDto>("add_dev_to_project", { project_idx, dev_id, add });

export const addWorker = (name: string) => invoke<AppStateDto>("add_worker", { name });

export const addDev = (name: string) => invoke<AppStateDto>("add_dev", { name });

export const setWorkerMaxHours = (worker_idx: number, hours: number) =>
  invoke<AppStateDto>("set_worker_max_hours", { worker_idx, hours });

export const setWorkerWeekOverride = (worker_idx: number, week: number, hours: number) =>
  invoke<AppStateDto>("set_worker_week_override", { worker_idx, week, hours });

export const searchWorker = (name: string) =>
  invoke<AppStateDto>("search_worker", { name });

export const saveFile = (path: string) => invoke<void>("save_file", { path });

export const openFile = (path: string) => invoke<AppStateDto>("open_file", { path });

export const findCompletions = (prefix: string) =>
  invoke<string[]>("find_completions", { prefix });

export async function pickOpenFile(): Promise<string | null> {
  const result = await open({
    filters: [{ name: "RON", extensions: ["ron"] }],
    multiple: false,
  });
  if (typeof result === "string") return result;
  return null;
}

export async function pickSaveFile(defaultPath?: string): Promise<string | null> {
  const result = await save({
    filters: [{ name: "RON", extensions: ["ron"] }],
    defaultPath,
  });
  return result ?? null;
}
