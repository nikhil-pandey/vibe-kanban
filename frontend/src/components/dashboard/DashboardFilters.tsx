import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import type { TaskStatus } from 'shared/types';

const ALL_STATUSES: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

const STATUS_LABELS: Record<TaskStatus, string> = {
  todo: 'Todo',
  inprogress: 'In Progress',
  inreview: 'In Review',
  done: 'Done',
  cancelled: 'Cancelled',
};

export const DEFAULT_STATUS_FILTERS: TaskStatus[] = ['todo', 'inprogress', 'inreview'];

interface DashboardFiltersProps {
  selectedStatuses: TaskStatus[];
  onStatusChange: (statuses: TaskStatus[]) => void;
}

export function DashboardFilters({
  selectedStatuses,
  onStatusChange,
}: DashboardFiltersProps) {
  const { t } = useTranslation(['tasks', 'common']);

  const handleToggleStatus = useCallback(
    (status: TaskStatus) => {
      if (selectedStatuses.includes(status)) {
        onStatusChange(selectedStatuses.filter((s) => s !== status));
      } else {
        onStatusChange([...selectedStatuses, status]);
      }
    },
    [selectedStatuses, onStatusChange]
  );

  const handleClearFilters = useCallback(() => {
    onStatusChange(DEFAULT_STATUS_FILTERS);
  }, [onStatusChange]);

  const isDefaultFilters = useMemo(() => {
    if (selectedStatuses.length !== DEFAULT_STATUS_FILTERS.length) return false;
    return DEFAULT_STATUS_FILTERS.every((s) => selectedStatuses.includes(s));
  }, [selectedStatuses]);

  return (
    <div className="flex items-center gap-4 flex-wrap">
      <span className="text-sm font-medium text-muted-foreground">
        {t('common:labels.filter', { defaultValue: 'Filter' })}:
      </span>
      <div className="flex items-center gap-3 flex-wrap">
        {ALL_STATUSES.map((status) => (
          <label
            key={status}
            className="flex items-center gap-1.5 cursor-pointer text-sm"
          >
            <Checkbox
              checked={selectedStatuses.includes(status)}
              onCheckedChange={() => handleToggleStatus(status)}
            />
            {STATUS_LABELS[status]}
          </label>
        ))}
      </div>
      {!isDefaultFilters && (
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs gap-1"
          onClick={handleClearFilters}
        >
          <X className="h-3 w-3" />
          {t('common:buttons.reset', { defaultValue: 'Reset' })}
        </Button>
      )}
    </div>
  );
}
