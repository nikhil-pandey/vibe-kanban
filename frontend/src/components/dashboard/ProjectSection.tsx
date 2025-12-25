import { useState, useCallback } from 'react';
import { ChevronDown, ChevronRight, ExternalLink } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { paths } from '@/lib/paths';
import { TaskTable } from './TaskTable';
import type { TaskWithAttemptStatusAndProject } from 'shared/types';

interface ProjectSectionProps {
  projectId: string;
  projectName: string;
  tasks: TaskWithAttemptStatusAndProject[];
  selectedTaskId?: string;
  onSelectTask: (task: TaskWithAttemptStatusAndProject) => void;
  defaultExpanded?: boolean;
}

export function ProjectSection({
  projectId,
  projectName,
  tasks,
  selectedTaskId,
  onSelectTask,
  defaultExpanded = true,
}: ProjectSectionProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const navigate = useNavigate();

  const handleToggle = useCallback(() => {
    setIsExpanded((prev) => !prev);
  }, []);

  const handleOpenProject = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      navigate(paths.projectTasks(projectId));
    },
    [navigate, projectId]
  );

  return (
    <div className="border rounded-lg bg-card mb-3">
      {/* Project Header */}
      <div
        className="flex items-center justify-between px-4 py-3 cursor-pointer hover:bg-muted/50 select-none"
        onClick={handleToggle}
      >
        <div className="flex items-center gap-2">
          {isExpanded ? (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
          )}
          <span className="font-medium">{projectName}</span>
          <span className="text-sm text-muted-foreground">
            ({tasks.length} {tasks.length === 1 ? 'task' : 'tasks'})
          </span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="h-8 gap-1 text-muted-foreground hover:text-foreground"
          onClick={handleOpenProject}
        >
          Open
          <ExternalLink className="h-3.5 w-3.5" />
        </Button>
      </div>

      {/* Task Table */}
      {isExpanded && tasks.length > 0 && (
        <div className="px-4 pb-3">
          <TaskTable
            tasks={tasks}
            selectedTaskId={selectedTaskId}
            onSelectTask={onSelectTask}
          />
        </div>
      )}
    </div>
  );
}
