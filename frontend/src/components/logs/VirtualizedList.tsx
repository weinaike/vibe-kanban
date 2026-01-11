import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { useEffect, useRef, useState } from 'react';

import DisplayConversationEntry from '../NormalizedConversation/DisplayConversationEntry';
import { useEntries } from '@/contexts/EntriesContext';
import {
  AddEntryType,
  PatchTypeWithKey,
  useConversationHistory,
} from '@/hooks/useConversationHistory';
import { Loader2 } from 'lucide-react';
import { TaskWithAttemptStatus } from 'shared/types';
import type { WorkspaceWithSession } from '@/types/attempt';
import { ApprovalFormProvider } from '@/contexts/ApprovalFormContext';

interface VirtualizedListProps {
  attempt: WorkspaceWithSession;
  task?: TaskWithAttemptStatus;
}
const VirtualizedList = ({ attempt, task }: VirtualizedListProps) => {
  const [entries, setEntriesState] = useState<PatchTypeWithKey[]>([]);
  const [loading, setLoading] = useState(true);
  const [shouldScrollToBottom, setShouldScrollToBottom] = useState(true);
  const { setEntries, reset } = useEntries();

  useEffect(() => {
    setLoading(true);
    setEntriesState([]);
    setShouldScrollToBottom(true);
    reset();
  }, [attempt.id, reset]);

  const onEntriesUpdated = (
    newEntries: PatchTypeWithKey[],
    addType: AddEntryType,
    newLoading: boolean
  ) => {
    setEntriesState(newEntries);
    setEntries(newEntries);

    // Auto scroll to bottom when new entries are added while running
    if (addType === 'running' && !loading) {
      setShouldScrollToBottom(true);
    } else if (addType === 'initial') {
      setShouldScrollToBottom(true);
    }

    if (loading) {
      setLoading(newLoading);
    }
  };

  useConversationHistory({ attempt, onEntriesUpdated });

  const virtuosoRef = useRef<VirtuosoHandle>(null);

  // Auto scroll to bottom when new entries are added
  useEffect(() => {
    if (shouldScrollToBottom && entries.length > 0 && !loading) {
      virtuosoRef.current?.scrollToIndex({
        index: entries.length - 1,
        behavior: 'smooth',
        align: 'end',
      });
      setShouldScrollToBottom(false);
    }
  }, [shouldScrollToBottom, entries.length, loading]);

  const itemContent = (_index: number, data: PatchTypeWithKey) => {
    if (data.type === 'STDOUT') {
      return <p>{data.content}</p>;
    }
    if (data.type === 'STDERR') {
      return <p>{data.content}</p>;
    }
    if (data.type === 'NORMALIZED_ENTRY' && attempt) {
      return (
        <DisplayConversationEntry
          expansionKey={data.patchKey}
          entry={data.content}
          executionProcessId={data.executionProcessId}
          taskAttempt={attempt}
          task={task}
        />
      );
    }

    return null;
  };

  const computeItemKey = (_index: number, data: PatchTypeWithKey) =>
    `l-${data.patchKey}`;

  return (
    <ApprovalFormProvider>
      <Virtuoso
        ref={virtuosoRef}
        className="flex-1"
        data={entries}
        itemContent={itemContent}
        computeItemKey={computeItemKey}
        components={{
          Header: () => <div className="h-2"></div>,
          Footer: () => <div className="h-2"></div>,
        }}
        initialTopMostItemIndex={entries.length > 0 ? entries.length - 1 : 0}
        followOutput="smooth"
      />
      {loading && (
        <div className="absolute top-0 left-0 w-full h-full bg-primary flex flex-col gap-2 justify-center items-center z-10">
          <Loader2 className="h-8 w-8 animate-spin" />
          <p>Loading History</p>
        </div>
      )}
    </ApprovalFormProvider>
  );
};

export default VirtualizedList;
