// Simplified version for local-only mode (shared tasks removed)

interface UseAssigneeUserNamesOptions {
  projectId: string | undefined;
  sharedTasks?: unknown[];
}

export function useAssigneeUserNames(_options: UseAssigneeUserNamesOptions) {
  // Local-only mode: no shared task assignees
  return {
    assignees: [] as Array<{ user_id: string; username?: string; email: string }>,
    refetchAssignees: () => {},
  };
}
