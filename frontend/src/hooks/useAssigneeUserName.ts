// Simplified version for local-only mode (shared tasks removed)

interface UseAssigneeUserNamesOptions {
  projectId: string | undefined;
  sharedTasks?: unknown[];
}

export function useAssigneeUserNames(options: UseAssigneeUserNamesOptions) {
  // Local-only mode: no shared task assignees
  // Explicitly destructure to mark as intentionally unused
  const { projectId, sharedTasks } = options;
  return {
    assignees: [] as Array<{ user_id: string; username?: string; email: string }>,
    refetchAssignees: () => {},
  };
}
