import { cn } from '@/lib/utils';
import { statusLabels, statusBoardColors } from '@/utils/statusLabels';
import type { TaskStatus } from 'shared/types';

interface StatusBadgeProps {
  status: TaskStatus;
  className?: string;
  showLabel?: boolean;
  size?: 'sm' | 'md';
}

export function StatusBadge({
  status,
  className,
  showLabel = true,
  size = 'md',
}: StatusBadgeProps) {
  const colorVar = statusBoardColors[status];
  const label = statusLabels[status];

  return (
    <div
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full font-medium transition-colors',
        size === 'sm' ? 'px-2 py-0.5 text-xs' : 'px-2.5 py-1 text-xs',
        className
      )}
      style={{
        backgroundColor: `hsl(var(${colorVar}) / 0.12)`,
        color: `hsl(var(${colorVar}))`,
      }}
    >
      <span
        className={cn(
          'rounded-full shrink-0',
          size === 'sm' ? 'h-1.5 w-1.5' : 'h-2 w-2'
        )}
        style={{
          backgroundColor: `hsl(var(${colorVar}))`,
        }}
      />
      {showLabel && <span>{label}</span>}
    </div>
  );
}

export function getStatusColor(status: TaskStatus): string {
  return statusBoardColors[status];
}
