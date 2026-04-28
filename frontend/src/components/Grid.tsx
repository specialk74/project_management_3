import { createSignal, For, Show, Setter } from "solid-js";
import type { AppStateDto, CellDto, DevDataDto, ProjectDto } from "../types";
import * as api from "../api";
import NoteDialog from "./NoteDialog";
import HoursDialog from "./HoursDialog";
import ContextMenu from "./ContextMenu";

interface Props {
  state: AppStateDto;
  onStateChange: Setter<AppStateDto | null>;
}

interface CtxState {
  x: number;
  y: number;
  items: { label: string; action: () => void }[];
}

interface NoteState {
  title: string;
  initial: string;
  onConfirm: (note: string) => void;
}

interface HoursState {
  title: string;
  initial: number;
  onConfirm: (hours: number) => void;
}

export default function Grid(props: Props) {
  let leftBodyRef: HTMLDivElement | undefined;
  let rightBodyRef: HTMLDivElement | undefined;

  const [ctx, setCtx] = createSignal<CtxState | null>(null);
  const [noteDialog, setNoteDialog] = createSignal<NoteState | null>(null);
  const [hoursDialog, setHoursDialog] = createSignal<HoursState | null>(null);

  const state = () => props.state;

  function isThisWeek(week: number) {
    return week === state().this_week;
  }

  function syncScroll(from: "left" | "right") {
    if (from === "left" && leftBodyRef && rightBodyRef) {
      rightBodyRef.scrollTop = leftBodyRef.scrollTop;
    } else if (from === "right" && leftBodyRef && rightBodyRef) {
      leftBodyRef.scrollTop = rightBodyRef.scrollTop;
    }
  }

  function closeCtx() {
    setCtx(null);
  }

  function openCtx(e: MouseEvent, items: CtxState["items"]) {
    e.preventDefault();
    setCtx({ x: e.clientX, y: e.clientY, items });
  }

  async function update(fn: () => Promise<AppStateDto>) {
    const newState = await fn();
    props.onStateChange(newState);
  }

  // ─── Cell editing ─────────────────────────────────────────

  const [editKey, setEditKey] = createSignal<string | null>(null);
  const [editValue, setEditValue] = createSignal("");

  function cellKey(pIdx: number, devId: number, week: number, row: number) {
    return `${pIdx}-${devId}-${week}-${row}`;
  }

  function startEdit(key: string, cell: CellDto) {
    const val = cell.worker_name
      ? `${cell.worker_name}|${cell.effort_pct}`
      : "";
    setEditKey(key);
    setEditValue(val);
  }

  async function commitEdit(
    pIdx: number,
    devId: number,
    week: number,
    rowIdx: number
  ) {
    const raw = editValue().trim();
    setEditKey(null);

    const weekData = state().projects[pIdx]?.dev_data.find((d) => d.dev_id === devId)
      ?.weeks.find((w) => w.week === week);
    if (!weekData) return;

    const cells = weekData.cells.map((c, i) => ({
      worker_name: c.worker_name,
      effort_pct: c.effort_pct,
    }));

    if (raw === "") {
      cells[rowIdx] = { worker_name: "", effort_pct: 0 };
    } else {
      const parts = raw.split("|");
      const workerName = parts[0].trim();
      const effort = parseInt(parts[1]?.trim() ?? "0", 10) || 0;
      cells[rowIdx] = { worker_name: workerName, effort_pct: effort };
    }

    await update(() => api.setCells(pIdx, devId, week, cells));
  }

  async function handleKeyDown(
    e: KeyboardEvent,
    pIdx: number,
    devId: number,
    week: number,
    rowIdx: number
  ) {
    if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      await commitEdit(pIdx, devId, week, rowIdx);
    } else if (e.key === "Escape") {
      setEditKey(null);
    } else if (e.key === "Tab" && editValue().includes("|") === false) {
      e.preventDefault();
      const prefix = editValue();
      if (prefix) {
        const completions = await api.findCompletions(prefix);
        if (completions.length === 1) {
          setEditValue(completions[0] + "|");
        } else if (completions.length > 1) {
          const common = commonPrefix(completions);
          if (common.length > prefix.length) setEditValue(common);
        }
      }
    }
  }

  function commonPrefix(strs: string[]): string {
    if (!strs.length) return "";
    let prefix = strs[0];
    for (const s of strs.slice(1)) {
      while (!s.startsWith(prefix)) prefix = prefix.slice(0, -1);
    }
    return prefix;
  }

  // ─── Cell colour ──────────────────────────────────────────

  function cellColor(cell: CellDto): string {
    if (!cell.worker_name) return "c-dim";
    if (cell.max_hours > 0 && cell.sovra_hours > cell.max_hours) return "c-red";
    if (cell.hours < 40) return "c-yellow";
    if (cell.hours < cell.max_hours) return "c-orange";
    return "c-green";
  }

  function sovraColor(hours: number, maxHours: number): string {
    if (hours > maxHours) return "c-red";
    if (hours < 40) return "c-yellow";
    if (hours < maxHours) return "c-orange";
    return "c-green";
  }

  // ─── Context menus ────────────────────────────────────────

  function onCellRightClick(
    e: MouseEvent,
    pIdx: number,
    devId: number,
    week: number,
    cell: CellDto,
    rowIdx: number
  ) {
    if (!cell.worker_name) return;
    openCtx(e, [
      {
        label: cell.note ? "Modifica nota..." : "Aggiungi nota...",
        action: () => {
          setNoteDialog({
            title: `Nota: ${cell.worker_name}`,
            initial: cell.note,
            onConfirm: async (note) => {
              await update(() =>
                api.setNote(pIdx, devId, week, cell.worker_name, note)
              );
              setNoteDialog(null);
            },
          });
          closeCtx();
        },
      },
    ]);
  }

  function onDevRightClick(e: MouseEvent, pIdx: number, dev: DevDataDto) {
    openCtx(e, [
      {
        label: dev.dev_note ? "Modifica nota dev..." : "Aggiungi nota dev...",
        action: () => {
          setNoteDialog({
            title: `Nota dev: ${dev.dev_name}`,
            initial: dev.dev_note,
            onConfirm: async (note) => {
              await update(() => api.setDevNote(pIdx, dev.dev_id, note));
              setNoteDialog(null);
            },
          });
          closeCtx();
        },
      },
      {
        label: "+ Riga",
        action: async () => {
          await update(() => api.addRow(pIdx, dev.dev_id));
          closeCtx();
        },
      },
      {
        label: "- Riga",
        action: async () => {
          await update(() => api.delRow(pIdx, dev.dev_id));
          closeCtx();
        },
      },
    ]);
  }

  function onWorkerRightClick(e: MouseEvent, workerIdx: number, workerName: string, currentMax: number) {
    openCtx(e, [
      {
        label: `Ore max (${currentMax}h)...`,
        action: () => {
          setHoursDialog({
            title: `Ore max: ${workerName}`,
            initial: currentMax,
            onConfirm: async (hours) => {
              await update(() => api.setWorkerMaxHours(workerIdx, hours));
              setHoursDialog(null);
            },
          });
          closeCtx();
        },
      },
    ]);
  }

  function onSovraRightClick(
    e: MouseEvent,
    workerIdx: number,
    workerName: string,
    week: number,
    currentMax: number
  ) {
    openCtx(e, [
      {
        label: `Override settimana (${currentMax}h)...`,
        action: () => {
          setHoursDialog({
            title: `Override settimana: ${workerName}`,
            initial: currentMax,
            onConfirm: async (hours) => {
              await update(() => api.setWorkerWeekOverride(workerIdx, week, hours));
              setHoursDialog(null);
            },
          });
          closeCtx();
        },
      },
    ]);
  }

  // ─── Project name editing ─────────────────────────────────

  async function handleProjectNameChange(pIdx: number, name: string) {
    await update(() => api.renameProject(pIdx, name));
  }

  // ─── Dev planned effort editing ───────────────────────────

  async function handleDevEffortChange(pIdx: number, devId: number, val: string) {
    const effort = parseInt(val, 10);
    if (!isNaN(effort)) {
      await update(() => api.setDevEffort(pIdx, devId, effort));
    }
  }

  // ─── Render ───────────────────────────────────────────────

  const weeks = () => state().weeks;

  function rowsForDev(dev: DevDataDto): number {
    return dev.weeks[0]?.cells.length ?? 1;
  }

  return (
    <div class="grid-outer" onClick={closeCtx}>
      {/* ── Header ── */}
      <div class="header-row" style="position:absolute;top:0;left:0;right:0;z-index:10;display:flex;">
        <div class="header-left">Progetto / Funzione</div>
        <div class="header-weeks" style="flex:1;overflow:hidden;" id="header-weeks">
          <For each={weeks()}>
            {(w) => (
              <div class={`week-header-cell${isThisWeek(w.week) ? " this-week" : ""}`}>
                {w.label}
              </div>
            )}
          </For>
        </div>
      </div>

      {/* ── Body ── */}
      <div style="display:flex;flex:1;margin-top:var(--cell-h);overflow:hidden;">
        {/* Left column */}
        <div class="grid-left">
          <div
            class="body-left"
            ref={leftBodyRef}
            style="overflow-y:auto;height:calc(100% - 60px)"
            onScroll={() => syncScroll("left")}
          >
            <For each={state().projects}>
              {(proj) => (
                <div class="project-block">
                  <div class="project-header-left">
                    <input
                      value={proj.name}
                      onBlur={(e) => handleProjectNameChange(proj.idx, e.currentTarget.value)}
                      onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
                      style={{ color: proj.enabled ? "var(--accent)" : "var(--text-dim)" }}
                    />
                  </div>
                  <For each={proj.dev_data}>
                    {(dev) => (
                      <>
                        <div
                          class="dev-row-left"
                          onContextMenu={(e) => onDevRightClick(e, proj.idx, dev)}
                        >
                          <span
                            class="dev-label"
                            style={{ background: "#2a2a2a", color: dev.enabled ? "var(--text)" : "var(--text-dim)" }}
                          >
                            {dev.dev_name}
                          </span>
                          <Show when={dev.dev_note}>
                            <span style="color:var(--yellow);font-size:9px;">●</span>
                          </Show>
                          <input
                            class="dev-effort-input"
                            value={dev.planned_hours}
                            onBlur={(e) =>
                              handleDevEffortChange(proj.idx, dev.dev_id, e.currentTarget.value)
                            }
                            onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
                          />
                          <span class="dev-total">{dev.total_hours}h</span>
                        </div>
                        <For each={Array.from({ length: rowsForDev(dev) })}>
                          {() => <div class="worker-row-left" />}
                        </For>
                      </>
                    )}
                  </For>
                </div>
              )}
            </For>
          </div>

          {/* Footer left: worker names */}
          <div class="footer-left" style="height:60px;overflow-y:hidden;">
            <For each={state().workers}>
              {(w) => (
                <div
                  class="footer-worker-row"
                  onContextMenu={(e) => onWorkerRightClick(e, w.idx, w.name, w.max_hours)}
                >
                  <Show when={w.max_hours < 40}>
                    <div class="tri-reduced" />
                  </Show>
                  <span class="footer-worker-name">{w.name}</span>
                  <Show when={w.max_hours < 40}>
                    <span class="footer-worker-max">{w.max_hours}h</span>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </div>

        {/* Right scrollable area */}
        <div class="grid-right">
          <div
            class="body-right"
            ref={rightBodyRef}
            style="overflow:auto;height:calc(100% - 60px)"
            onScroll={(e) => {
              syncScroll("right");
              // sync header
              const hw = document.getElementById("header-weeks");
              if (hw) hw.scrollLeft = (e.target as HTMLElement).scrollLeft;
            }}
          >
            <For each={state().projects}>
              {(proj) => (
                <div class="project-block">
                  {/* project header row (same height as left) */}
                  <div class="cell-row">
                    <For each={weeks()}>
                      {(w) => (
                        <div class={`dev-week-cell${isThisWeek(w.week) ? " this-week-col" : ""}`} />
                      )}
                    </For>
                  </div>
                  <For each={proj.dev_data}>
                    {(dev) => (
                      <>
                        {/* dev totals row */}
                        <div class="cell-row">
                          <For each={dev.weeks}>
                            {(wk) => (
                              <div class={`dev-week-cell${isThisWeek(wk.week) ? " this-week-col" : ""}`}>
                                {wk.total_hours > 0 ? wk.total_hours : ""}
                              </div>
                            )}
                          </For>
                        </div>
                        {/* worker rows */}
                        <For each={Array.from({ length: rowsForDev(dev) }, (_, i) => i)}>
                          {(rowIdx) => (
                            <div class="cell-row">
                              <For each={dev.weeks}>
                                {(wk) => {
                                  const cell = () => wk.cells[rowIdx] ?? { worker_name: "", effort_pct: 0, hours: 0, sovra_hours: 0, max_hours: 40, note: "" };
                                  const key = () => cellKey(proj.idx, dev.dev_id, wk.week, rowIdx);
                                  const isEditing = () => editKey() === key();
                                  return (
                                    <div
                                      class={`week-cell${isThisWeek(wk.week) ? " this-week-col" : ""}${isEditing() ? " editing" : ""}`}
                                      onClick={() => startEdit(key(), cell())}
                                      onContextMenu={(e) =>
                                        onCellRightClick(e, proj.idx, dev.dev_id, wk.week, cell(), rowIdx)
                                      }
                                    >
                                      <Show when={isEditing()} fallback={
                                        <span class={cellColor(cell())}>
                                          {cell().worker_name
                                            ? `${cell().worker_name}|${cell().effort_pct}`
                                            : ""}
                                        </span>
                                      }>
                                        <input
                                          autofocus
                                          value={editValue()}
                                          onInput={(e) => setEditValue(e.currentTarget.value)}
                                          onBlur={() => commitEdit(proj.idx, dev.dev_id, wk.week, rowIdx)}
                                          onKeyDown={(e) => handleKeyDown(e, proj.idx, dev.dev_id, wk.week, rowIdx)}
                                          onClick={(e) => e.stopPropagation()}
                                        />
                                      </Show>
                                      <Show when={cell().note && !isEditing()}>
                                        <span class="note-dot" />
                                      </Show>
                                    </div>
                                  );
                                }}
                              </For>
                            </div>
                          )}
                        </For>
                      </>
                    )}
                  </For>
                </div>
              )}
            </For>
          </div>

          {/* Footer right: sovra */}
          <div class="footer-right" style="height:60px;overflow-x:hidden;overflow-y:hidden;">
            <For each={state().workers}>
              {(worker) => {
                return (
                  <div class="footer-cell-row">
                    <For each={state().sovra}>
                      {(sw) => {
                        const sw_worker = () =>
                          sw.workers.find((w) => w.worker_idx === worker.idx) ?? {
                            hours: 0,
                            max_hours: 40,
                            name: worker.name,
                            worker_idx: worker.idx,
                          };
                        return (
                          <div
                            class={`footer-week-cell${isThisWeek(sw.week) ? " this-week-col" : ""} ${sovraColor(sw_worker().hours, sw_worker().max_hours)}`}
                            onContextMenu={(e) =>
                              onSovraRightClick(
                                e,
                                worker.idx,
                                worker.name,
                                sw.week,
                                sw_worker().max_hours
                              )
                            }
                          >
                            {sw_worker().hours > 0 ? sw_worker().hours : ""}
                          </div>
                        );
                      }}
                    </For>
                  </div>
                );
              }}
            </For>
          </div>
        </div>
      </div>

      {/* ── Overlays ── */}
      <Show when={ctx()}>
        {(c) => (
          <ContextMenu
            x={c().x}
            y={c().y}
            items={c().items}
            onClose={closeCtx}
          />
        )}
      </Show>

      <Show when={noteDialog()}>
        {(d) => (
          <NoteDialog
            title={d().title}
            initial={d().initial}
            onConfirm={d().onConfirm}
            onCancel={() => setNoteDialog(null)}
          />
        )}
      </Show>

      <Show when={hoursDialog()}>
        {(d) => (
          <HoursDialog
            title={d().title}
            initial={d().initial}
            onConfirm={d().onConfirm}
            onCancel={() => setHoursDialog(null)}
          />
        )}
      </Show>
    </div>
  );
}
