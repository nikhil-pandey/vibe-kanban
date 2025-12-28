import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2, XCircle, MoreHorizontal, Pencil, Trash2, Play, Square } from 'lucide-react';
import { cn } from '@/lib/utils';
import {
  Table,
  TableHead,
  TableBody,
  TableRow,
  TableHeaderCell,
  TableCell,
} from '@/components/ui/table/table';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { StatusBadge, getStatusColor } from '@/components/ui/StatusBadge';
import { Checkbox } from '@/components/ui/checkbox';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';
import { tasksApi, attemptsApi } from '@/lib/api';
import { openTaskForm } from '@/lib/openTaskForm';
import type { TaskWithAttemptStatusAndProject, TaskStatus, Workspace } from 'shared/types';

const TASK_STATUSES: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

interface TaskTableProps {
  tasks: TaskWithAttemptStatusAndProject[];
  selectedTaskId?: string;
  onSelectTask: (task: TaskWithAttemptStatusAndProject) => void;
  onAttemptCreated?: (task: TaskWithAttemptStatusAndProject, attempt: Workspace) => void;
  onRefresh?: () => void;
  // Bulk selection props
  selectedTaskIds: Set<string>;
  onToggleTaskSelection: (taskId: string) => void;
}

export function TaskTable({
  tasks,
  selectedTaskId,
  onSelectTask,
  onAttemptCreated,
  onRefresh,
  selectedTaskIds,
  onToggleTaskSelection,
}: TaskTableProps) {
  const { t } = useTranslation(['tasks', 'common']);
  const [stoppingTaskId, setStoppingTaskId] = useState<string | null>(null);

  const handleStartAttempt = useCallback(
    async (e: React.MouseEvent, task: TaskWithAttemptStatusAndProject) => {
      e.stopPropagation();
      // Use the dialog's static show method, passing projectId since we're not within a ProjectProvider
      // Provide onSuccess callback to stay on dashboard and open the attempt in sidebar
      await CreateAttemptDialog.show({
        taskId: task.id,
        projectId: task.project_id,
        onSuccess: onAttemptCreated
          ? (attempt) => onAttemptCreated(task, attempt)
          : onRefresh
            ? () => onRefresh()
            : undefined,
      });
    },
    [onAttemptCreated, onRefresh]
  );

  const handleStopAttempt = useCallback(
    async (e: React.MouseEvent, task: TaskWithAttemptStatusAndProject) => {
      e.stopPropagation();
      try {
        setStoppingTaskId(task.id);
        // Fetch attempts to find the running one
        const attempts = await attemptsApi.getAll(task.id);
        // Get the most recent attempt (sorted by created_at desc)
        const sortedAttempts = [...attempts].sort(
          (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
        const runningAttempt = sortedAttempts[0];

        if (runningAttempt) {
          await attemptsApi.stop(runningAttempt.id);
        }
      } catch (err) {
        console.error('Failed to stop attempt:', err);
      } finally {
        setStoppingTaskId(null);
        onRefresh?.();
      }
    },
    [onRefresh]
  );

  const handleStatusChange = useCallback(
    async (taskId: string, newStatus: TaskStatus) => {
      try {
        await tasksApi.update(taskId, {
          title: null,
          description: null,
          status: newStatus,
          parent_workspace_id: null,
          image_ids: null,
        });
        onRefresh?.();
      } catch (err) {
        console.error('Failed to update task status:', err);
      }
    },
    [onRefresh]
  );

  const handleEdit = useCallback(
    (e: React.MouseEvent, task: TaskWithAttemptStatusAndProject) => {
      e.stopPropagation();
      openTaskForm({
        mode: 'edit',
        projectId: task.project_id,
        task,
        onSuccess: onRefresh,
      });
    },
    [onRefresh]
  );

  const handleDelete = useCallback(
    async (e: React.MouseEvent, task: TaskWithAttemptStatusAndProject) => {
      e.stopPropagation();
      if (window.confirm(t('tasks:deleteConfirm', { defaultValue: 'Are you sure you want to delete this task?' }))) {
        try {
          await tasksApi.delete(task.id);
          onRefresh?.();
        } catch (err) {
          console.error('Failed to delete task:', err);
        }
      }
    },
    [onRefresh, t]
  );

  if (tasks.length === 0) {
    return (
      <div className="text-sm text-muted-foreground py-4 px-2">
        {t('tasks:empty.noTasks', { defaultValue: 'No tasks found' })}
      </div>
    );
  }

  return (
    <Table>
      <TableHead>
        <TableRow>
          <TableHeaderCell className="py-2 px-2 w-10">
            <Checkbox
              checked={tasks.length > 0 && tasks.every(t => selectedTaskIds.has(t.id))}
              onCheckedChange={(checked) => {
                tasks.forEach(t => {
                  const isSelected = selectedTaskIds.has(t.id);
                  if ((checked && !isSelected) || (!checked && isSelected)) {
                    onToggleTaskSelection(t.id);
                  }
                });
              }}
            />
          </TableHeaderCell>
          <TableHeaderCell className="py-2 px-2">{t('common:labels.title', { defaultValue: 'Title' })}</TableHeaderCell>
          <TableHeaderCell className="py-2 px-2 w-36">{t('common:labels.status', { defaultValue: 'Status' })}</TableHeaderCell>
          <TableHeaderCell className="py-2 px-2 w-10"></TableHeaderCell>
          <TableHeaderCell className="py-2 px-2 w-10"></TableHeaderCell>
        </TableRow>
      </TableHead>
      <TableBody>
        {tasks.map((task) => (
          <TableRow
            key={task.id}
            clickable
            onClick={() => onSelectTask(task)}
            className={cn(
              task.id === selectedTaskId && 'bg-muted',
              selectedTaskIds.has(task.id) && 'bg-primary/10'
            )}
            style={{
              borderLeft: `4px solid hsl(var(${getStatusColor(task.status)}))`,
            }}
          >
            <TableCell className="py-2 px-2" onClick={(e) => e.stopPropagation()}>
              <Checkbox
                checked={selectedTaskIds.has(task.id)}
                onCheckedChange={() => onToggleTaskSelection(task.id)}
              />
            </TableCell>
            <TableCell className="py-2 px-2">
              <div className="flex items-center gap-2">
                <span className="truncate max-w-md">{task.title}</span>
                {task.has_in_progress_attempt && (
                  <Loader2 className="h-3.5 w-3.5 animate-spin text-blue-500 shrink-0" />
                )}
                {task.last_attempt_failed && (
                  <XCircle className="h-3.5 w-3.5 text-destructive shrink-0" />
                )}
              </div>
            </TableCell>
            <TableCell className="py-2 px-2">
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button
                    type="button"
                    onClick={(e) => e.stopPropagation()}
                    className="cursor-pointer hover:opacity-80 transition-opacity"
                  >
                    <StatusBadge status={task.status} size="sm" />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="w-40">
                  {TASK_STATUSES.map((status) => (
                    <DropdownMenuItem
                      key={status}
                      onClick={() => handleStatusChange(task.id, status)}
                      className="p-0"
                    >
                      <StatusBadge status={status} size="sm" />
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            </TableCell>
            <TableCell className="py-2 px-1">
              <TooltipProvider>
                {task.has_in_progress_attempt ? (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="icon"
                        className="h-7 w-7 text-destructive hover:text-destructive hover:bg-destructive/10"
                        onClick={(e) => handleStopAttempt(e, task)}
                        disabled={stoppingTaskId === task.id}
                      >
                        {stoppingTaskId === task.id ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />
                        ) : (
                          <Square className="h-3.5 w-3.5 fill-current" />
                        )}
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t('tasks:actions.stopAttempt', { defaultValue: 'Stop attempt' })}
                    </TooltipContent>
                  </Tooltip>
                ) : (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="icon"
                        className="h-7 w-7 text-success hover:text-success hover:bg-success/10"
                        onClick={(e) => handleStartAttempt(e, task)}
                      >
                        <Play className="h-3.5 w-3.5 fill-current" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t('tasks:actions.startAttempt', { defaultValue: 'Start attempt' })}
                    </TooltipContent>
                  </Tooltip>
                )}
              </TooltipProvider>
            </TableCell>
            <TableCell className="py-2 px-2">
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="icon"
                    className="h-8 w-8"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <MoreHorizontal className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem onClick={(e) => handleEdit(e as unknown as React.MouseEvent, task)}>
                    <Pencil className="h-4 w-4 mr-2" />
                    {t('common:buttons.edit', { defaultValue: 'Edit' })}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={(e) => handleDelete(e as unknown as React.MouseEvent, task)}
                    className="text-destructive"
                  >
                    <Trash2 className="h-4 w-4 mr-2" />
                    {t('common:buttons.delete', { defaultValue: 'Delete' })}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
