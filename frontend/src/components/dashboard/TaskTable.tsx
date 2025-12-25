import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2, XCircle, MoreHorizontal, Pencil, Trash2 } from 'lucide-react';
import {
  Table,
  TableHead,
  TableBody,
  TableRow,
  TableHeaderCell,
  TableCell,
} from '@/components/ui/table/table';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import { tasksApi } from '@/lib/api';
import { openTaskForm } from '@/lib/openTaskForm';
import type { TaskWithAttemptStatusAndProject, TaskStatus } from 'shared/types';

const TASK_STATUSES: TaskStatus[] = [
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

interface TaskTableProps {
  tasks: TaskWithAttemptStatusAndProject[];
  selectedTaskId?: string;
  onSelectTask: (task: TaskWithAttemptStatusAndProject) => void;
}

export function TaskTable({
  tasks,
  selectedTaskId,
  onSelectTask,
}: TaskTableProps) {
  const { t } = useTranslation(['tasks', 'common']);

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
      } catch (err) {
        console.error('Failed to update task status:', err);
      }
    },
    []
  );

  const handleEdit = useCallback(
    (e: React.MouseEvent, task: TaskWithAttemptStatusAndProject) => {
      e.stopPropagation();
      openTaskForm({ mode: 'edit', projectId: task.project_id, task });
    },
    []
  );

  const handleDelete = useCallback(
    async (e: React.MouseEvent, task: TaskWithAttemptStatusAndProject) => {
      e.stopPropagation();
      if (window.confirm(t('tasks:deleteConfirm', { defaultValue: 'Are you sure you want to delete this task?' }))) {
        try {
          await tasksApi.delete(task.id);
        } catch (err) {
          console.error('Failed to delete task:', err);
        }
      }
    },
    [t]
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
          <TableHeaderCell className="py-2 px-2">{t('common:labels.title', { defaultValue: 'Title' })}</TableHeaderCell>
          <TableHeaderCell className="py-2 px-2 w-36">{t('common:labels.status', { defaultValue: 'Status' })}</TableHeaderCell>
          <TableHeaderCell className="py-2 px-2 w-12"></TableHeaderCell>
        </TableRow>
      </TableHead>
      <TableBody>
        {tasks.map((task) => (
          <TableRow
            key={task.id}
            clickable
            onClick={() => onSelectTask(task)}
            className={task.id === selectedTaskId ? 'bg-muted' : ''}
          >
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
              <Select
                value={task.status}
                onValueChange={(value) => handleStatusChange(task.id, value as TaskStatus)}
              >
                <SelectTrigger
                  className="h-8 text-xs w-full"
                  onClick={(e) => e.stopPropagation()}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {TASK_STATUSES.map((status) => (
                    <SelectItem key={status} value={status}>
                      {STATUS_LABELS[status]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
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
