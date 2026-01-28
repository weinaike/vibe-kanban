// Simplified version for local-only mode (shared tasks removed)

export function useAssigneeUserNames() {
  // Local-only mode: no shared task assignees
  return {
    assignees: [] as Array<{ user_id: string; username?: string; email: string }>,
    refetchAssignees: () => {},
  };
}
