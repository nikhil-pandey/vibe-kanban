import { useCallback, useMemo } from 'react';
import { useJsonPatchWsStream } from './useJsonPatchWsStream';
import type {
  TaskStatus,
  TaskWithAttemptStatusAndProject,
} from 'shared/types';

type AllTasksState = {
  tasks: Record<string, TaskWithAttemptStatusAndProject>;
};

export interface UseAllTasksResult {
  tasks: TaskWithAttemptStatusAndProject[];
  tasksById: Record<string, TaskWithAttemptStatusAndProject>;
  tasksByStatus: Record<TaskStatus, TaskWithAttemptStatusAndProject[]>;
  tasksByProject: Record<string, TaskWithAttemptStatusAndProject[]>;
  projectNames: { id: string; name: string }[];
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
  /** Force reconnect and fetch fresh data */
  refresh: () => void;
}

/**
 * Stream all tasks across all projects via WebSocket (JSON Patch).
 * Server sends initial snapshot: replace /tasks with an object keyed by id.
 * Live updates arrive at /tasks/<id> via add/replace/remove operations.
 */
export const useAllTasks = (): UseAllTasksResult => {
  const endpoint = '/api/all-tasks/stream/ws';

  const initialData = useCallback((): AllTasksState => ({ tasks: {} }), []);

  const { data, isConnected, error, refresh } = useJsonPatchWsStream(
    endpoint,
    true, // always enabled
    initialData
  );

  const localTasksById = useMemo(() => data?.tasks ?? {}, [data?.tasks]);

  const { tasks, tasksById, tasksByStatus, tasksByProject, projectNames } = useMemo(() => {
    const merged: Record<string, TaskWithAttemptStatusAndProject> = { ...localTasksById };
    const byStatus: Record<TaskStatus, TaskWithAttemptStatusAndProject[]> = {
      todo: [],
      inprogress: [],
      inreview: [],
      done: [],
      cancelled: [],
    };
    const byProject: Record<string, TaskWithAttemptStatusAndProject[]> = {};
    const projectMap: Map<string, string> = new Map(); // id -> name

    Object.values(merged).forEach((task) => {
      byStatus[task.status]?.push(task);

      if (!byProject[task.project_id]) {
        byProject[task.project_id] = [];
      }
      byProject[task.project_id].push(task);

      // Track project names
      if (!projectMap.has(task.project_id)) {
        projectMap.set(task.project_id, task.project_name);
      }
    });

    // Sort tasks within each bucket by created_at descending
    const sortByCreatedAt = (a: TaskWithAttemptStatusAndProject, b: TaskWithAttemptStatusAndProject) =>
      new Date(b.created_at).getTime() - new Date(a.created_at).getTime();

    const sorted = Object.values(merged).sort(sortByCreatedAt);

    (Object.values(byStatus) as TaskWithAttemptStatusAndProject[][]).forEach((list) => {
      list.sort(sortByCreatedAt);
    });

    Object.values(byProject).forEach((list) => {
      list.sort(sortByCreatedAt);
    });

    // Create sorted list of project names
    const projectNamesList = Array.from(projectMap.entries())
      .map(([id, name]) => ({ id, name }))
      .sort((a, b) => a.name.localeCompare(b.name));

    return {
      tasks: sorted,
      tasksById: merged,
      tasksByStatus: byStatus,
      tasksByProject: byProject,
      projectNames: projectNamesList,
    };
  }, [localTasksById]);

  const isLoading = !data && !error; // until first snapshot

  return {
    tasks,
    tasksById,
    tasksByStatus,
    tasksByProject,
    projectNames,
    isLoading,
    isConnected,
    error,
    refresh,
  };
};
