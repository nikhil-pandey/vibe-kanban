import { useState, useCallback, useEffect } from 'react';
import { taskQueueApi } from '@/lib/api';
import type { SessionQueueStatus, TaskQueueEntry, QueuePosition } from 'shared/types';

interface UseTaskQueueStatusResult {
  /** Current task queue status */
  status: SessionQueueStatus | null;
  /** Whether the session has a pending entry in the task queue */
  isQueued: boolean;
  /** The queue entry if any */
  entry: TaskQueueEntry | null;
  /** Position info if queued */
  position: QueuePosition | null;
  /** Whether an operation is in progress */
  isLoading: boolean;
  /** Cancel the queued task */
  cancelQueue: () => Promise<void>;
  /** Refresh the queue status from the server */
  refresh: () => Promise<void>;
}

export function useTaskQueueStatus(
  sessionId?: string,
  pollInterval?: number
): UseTaskQueueStatusResult {
  const [status, setStatus] = useState<SessionQueueStatus | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const refresh = useCallback(async () => {
    if (!sessionId) return;
    try {
      const result = await taskQueueApi.getStatus(sessionId);
      setStatus(result);
    } catch (e) {
      console.error('Failed to fetch task queue status:', e);
    }
  }, [sessionId]);

  const cancelQueue = useCallback(async () => {
    if (!sessionId) return;
    setIsLoading(true);
    try {
      const result = await taskQueueApi.cancel(sessionId);
      setStatus(result);
    } finally {
      setIsLoading(false);
    }
  }, [sessionId]);

  // Fetch initial status when sessionId changes
  useEffect(() => {
    if (sessionId) {
      refresh();
    } else {
      setStatus(null);
    }
  }, [sessionId, refresh]);

  // Poll for updates if interval is provided and session is queued
  useEffect(() => {
    if (!sessionId || !pollInterval || !status?.is_queued) return;

    const intervalId = setInterval(refresh, pollInterval);
    return () => clearInterval(intervalId);
  }, [sessionId, pollInterval, status?.is_queued, refresh]);

  const isQueued = status?.is_queued ?? false;
  const entry = status?.entry ?? null;
  const position = status?.position ?? null;

  return {
    status,
    isQueued,
    entry,
    position,
    isLoading,
    cancelQueue,
    refresh,
  };
}
