// Simplified version for local-only mode (shared tasks removed)
import type { UserData } from 'shared/types';

interface UseAssigneeUserNamesOptions {
  projectId: string | undefined;
  sharedTasks?: unknown[]; // Changed from SharedTask[] to unknown[]
}

export function useAssigneeUserNames(_options: UseAssigneeUserNamesOptions) {
  // Local-only mode: no shared task assignees
  return {
    assignees: [] as UserData[],
    refetchAssignees: () => {},
  };
}
