import { useMutation } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import type { PushTaskAttemptRequest, PushError } from 'shared/types';
import type { Result } from '@/lib/api';

export function useForcePush(
  attemptId: string | undefined,
  onSuccess?: () => void,
  onError?: (err: unknown) => void
) {
  return useMutation({
    mutationFn: async (params: PushTaskAttemptRequest) => {
      if (!attemptId) throw new Error('attemptId is required');
      return attemptsApi.forcePush(attemptId, params) as Promise<Result<void, PushError>>;
    },
    onSuccess,
    onError,
  });
}
