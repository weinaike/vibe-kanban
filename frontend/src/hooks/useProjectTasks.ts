// Local-only mode: fetch tasks via HTTP polling (no shared tasks)
import { useMemo, useState, useEffect } from 'react';
import type { TaskStatus, TaskWithAttemptStatus } from 'shared/types';

export type SharedTaskRecord = never; // No shared tasks in local mode

type TasksState = {
  tasks: TaskWithAttemptStatus[];
  tasksById: Record<string, TaskWithAttemptStatus>;
  tasksByStatus: Record<TaskStatus, TaskWithAttemptStatus[]>;
  sharedTasksById: Record<string, SharedTaskRecord>;
  sharedTasksList: unknown[];
};

export function useProjectTasks(projectId: string | undefined) {
  const [data, setData] = useState<{ tasks: TaskWithAttemptStatus[] }>({ tasks: [] });
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch tasks via HTTP polling
  useEffect(() => {
    if (!projectId) {
      setIsConnected(false);
      return;
    }

    setIsConnected(true);
    setError(null);

    const fetchTasks = async () => {
      try {
        const res = await fetch(`/api/tasks?project_id=${projectId}`);
        const result = await res.json();
        if (result.success && result.data) {
          setData({ tasks: result.data });
        }
      } catch (err) {
        console.error('Failed to fetch tasks:', err);
        setError(err instanceof Error ? err.message : 'Failed to fetch tasks');
      }
    };

    // Initial fetch
    fetchTasks();

    // Poll for updates every 5 seconds
    const interval = setInterval(fetchTasks, 5000);

    return () => {
      clearInterval(interval);
      setIsConnected(false);
    };
  }, [projectId]);

  // Process tasks into the expected format
  const { tasks, tasksById, tasksByStatus } = useMemo(() => {
    // Ensure tasks is always an array
    const rawData = data?.tasks;
    const taskList: TaskWithAttemptStatus[] = Array.isArray(rawData) ? rawData : [];
    const byId: Record<string, TaskWithAttemptStatus> = {};
    const byStatus: Record<TaskStatus, TaskWithAttemptStatus[]> = {
      todo: [],
      inprogress: [],
      inreview: [],
      done: [],
      cancelled: [],
    };

    taskList.forEach((task) => {
      byId[task.id] = task;
      if (task.status && byStatus[task.status]) {
        byStatus[task.status].push(task);
      }
    });

    const sorted = [...taskList].sort(
      (a, b) =>
        new Date(b.created_at as string).getTime() -
        new Date(a.created_at as string).getTime()
    );

    // Sort each status list by created_at
    Object.values(byStatus).forEach((list) => {
      list.sort(
        (a, b) =>
          new Date(b.created_at as string).getTime() -
          new Date(a.created_at as string).getTime()
      );
    });

    return {
      tasks: sorted,
      tasksById: byId,
      tasksByStatus: byStatus,
    };
  }, [data]);

  const sharedTasksById = useMemo(() => ({}), []);
  const sharedTasksList = useMemo(() => [], []);

  return {
    data: {
      tasks,
      tasksById,
      tasksByStatus,
      sharedTasksById,
      sharedTasksList,
    } as TasksState,
    isLoading: !isConnected && !error,
    error,
  };
}

export function useProjectTasksWs() {
  // This function is kept for compatibility but is no longer needed
  // useProjectTasks now handles WebSocket connection internally
  return {
    data: undefined,
    isLoading: false,
    error: null,
  };
}
