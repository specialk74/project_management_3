export interface WeekDto {
  week: number;
  label: string;
}

export interface CellDto {
  worker_name: string;
  effort_pct: number;
  hours: number;
  sovra_hours: number;
  max_hours: number;
  note: string;
}

export interface WeekCellsDto {
  week: number;
  total_hours: number;
  cells: CellDto[];
}

export interface DevDataDto {
  dev_id: number;
  dev_name: string;
  dev_note: string;
  planned_hours: number;
  total_hours: number;
  enabled: boolean;
  weeks: WeekCellsDto[];
}

export interface ProjectDto {
  idx: number;
  name: string;
  enabled: boolean;
  dev_data: DevDataDto[];
}

export interface WorkerDto {
  idx: number;
  name: string;
  max_hours: number;
}

export interface DevDto {
  id: number;
  name: string;
  bg_color: string;
  text_color: string;
}

export interface SovraWorkerDto {
  worker_idx: number;
  name: string;
  hours: number;
  max_hours: number;
}

export interface SovraWeekDto {
  week: number;
  workers: SovraWorkerDto[];
}

export interface AppStateDto {
  weeks: WeekDto[];
  projects: ProjectDto[];
  workers: WorkerDto[];
  devs: DevDto[];
  sovra: SovraWeekDto[];
  this_week: number;
  current_file: string;
  changed: boolean;
}

export interface CellUpdate {
  worker_name: string;
  effort_pct: number;
}
