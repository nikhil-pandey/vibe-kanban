import { useState, useCallback } from 'react';
import {
  ChevronDown,
  ChevronRight,
  Edit,
  ExternalLink,
  FolderOpen,
  Link2,
  MoreHorizontal,
  Plus,
  Trash2,
  Unlink,
} from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { paths } from '@/lib/paths';
import { projectsApi } from '@/lib/api';
import { TaskTable } from './TaskTable';
import { useOpenProjectInEditor } from '@/hooks/useOpenProjectInEditor';
import { useProjectRepos } from '@/hooks';
import { useProjectMutations } from '@/hooks/useProjectMutations';
import { LinkProjectDialog } from '@/components/dialogs/projects/LinkProjectDialog';
import { openTaskForm } from '@/lib/openTaskForm';
import type { Project, TaskWithAttemptStatusAndProject, Workspace } from 'shared/types';

interface ProjectSectionProps {
  projectId: string;
  projectName: string;
  project?: Project;
  tasks: TaskWithAttemptStatusAndProject[];
  selectedTaskId?: string;
  onSelectTask: (task: TaskWithAttemptStatusAndProject) => void;
  onAttemptCreated?: (task: TaskWithAttemptStatusAndProject, attempt: Workspace) => void;
  onRefresh?: () => void;
  defaultExpanded?: boolean;
  // Bulk selection props
  selectedTaskIds: Set<string>;
  onToggleTaskSelection: (taskId: string) => void;
}

export function ProjectSection({
  projectId,
  projectName,
  project,
  tasks,
  selectedTaskId,
  onSelectTask,
  onAttemptCreated,
  onRefresh,
  defaultExpanded = true,
  selectedTaskIds,
  onToggleTaskSelection,
}: ProjectSectionProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();
  const { t } = useTranslation(['projects', 'common', 'tasks']);

  // Get project repos to determine if it's a single-repo project
  const { data: repos } = useProjectRepos(projectId);
  const isSingleRepoProject = repos?.length === 1;

  // Hook for opening project in IDE
  const handleOpenInEditor = useOpenProjectInEditor(project ?? { id: projectId, name: projectName } as Project);

  // Project mutations
  const { unlinkProject } = useProjectMutations({
    onUnlinkError: (err) => {
      console.error('Failed to unlink project:', err);
      setError('Failed to unlink project');
    },
  });

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

  const handleViewProject = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      navigate(`/projects/${projectId}`);
    },
    [navigate, projectId]
  );

  const handleOpenInIDE = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      handleOpenInEditor();
    },
    [handleOpenInEditor]
  );

  const handleEdit = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      navigate(`/settings/projects?projectId=${projectId}`);
    },
    [navigate, projectId]
  );

  const handleDelete = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (
        !confirm(
          `Are you sure you want to delete "${projectName}"? This action cannot be undone.`
        )
      )
        return;

      try {
        await projectsApi.delete(projectId);
      } catch (err) {
        console.error('Failed to delete project:', err);
        setError('Failed to delete project');
      }
    },
    [projectId, projectName]
  );

  const handleCreateTask = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      openTaskForm({
        mode: 'create',
        projectId,
        navigateOnCreate: false,
        onSuccess: onRefresh,
      });
    },
    [projectId, onRefresh]
  );

  const handleLinkProject = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation();
      try {
        await LinkProjectDialog.show({
          projectId,
          projectName,
        });
      } catch (err) {
        console.error('Failed to link project:', err);
      }
    },
    [projectId, projectName]
  );

  const handleUnlinkProject = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      const confirmed = window.confirm(
        `Are you sure you want to unlink "${projectName}"? The local project will remain, but it will no longer be linked to the remote project.`
      );
      if (confirmed) {
        unlinkProject.mutate(projectId);
      }
    },
    [projectId, projectName, unlinkProject]
  );

  const handleDismissError = useCallback(() => {
    setError(null);
  }, []);

  return (
    <div className="border rounded-lg bg-card mb-3">
      {/* Error Alert */}
      {error && (
        <div className="px-4 py-2 bg-destructive/10 text-destructive text-sm flex items-center justify-between">
          <span>{error}</span>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2"
            onClick={handleDismissError}
          >
            Dismiss
          </Button>
        </div>
      )}

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
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            className="h-8 gap-1"
            onClick={handleCreateTask}
          >
            <Plus className="h-3.5 w-3.5" />
            {t('tasks.create', { defaultValue: 'New Task' })}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-8 gap-1 text-muted-foreground hover:text-foreground"
            onClick={handleOpenProject}
          >
            Open
            <ExternalLink className="h-3.5 w-3.5" />
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild onClick={(e) => e.stopPropagation()}>
              <Button variant="ghost" size="sm" className="h-8 w-8 p-0">
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={handleViewProject}>
                <ExternalLink className="mr-2 h-4 w-4" />
                {t('viewProject')}
              </DropdownMenuItem>
              {isSingleRepoProject && (
                <DropdownMenuItem onClick={handleOpenInIDE}>
                  <FolderOpen className="mr-2 h-4 w-4" />
                  {t('openInIDE')}
                </DropdownMenuItem>
              )}
              {project?.remote_project_id ? (
                <DropdownMenuItem onClick={handleUnlinkProject}>
                  <Unlink className="mr-2 h-4 w-4" />
                  {t('unlinkFromOrganization')}
                </DropdownMenuItem>
              ) : (
                <DropdownMenuItem onClick={handleLinkProject}>
                  <Link2 className="mr-2 h-4 w-4" />
                  {t('linkToOrganization')}
                </DropdownMenuItem>
              )}
              <DropdownMenuItem onClick={handleEdit}>
                <Edit className="mr-2 h-4 w-4" />
                {t('common:buttons.edit')}
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={handleDelete}
                className="text-destructive"
              >
                <Trash2 className="mr-2 h-4 w-4" />
                {t('common:buttons.delete')}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      {/* Task Table */}
      {isExpanded && tasks.length > 0 && (
        <div className="px-4 pb-3">
          <TaskTable
            tasks={tasks}
            selectedTaskId={selectedTaskId}
            onSelectTask={onSelectTask}
            onAttemptCreated={onAttemptCreated}
            onRefresh={onRefresh}
            selectedTaskIds={selectedTaskIds}
            onToggleTaskSelection={onToggleTaskSelection}
          />
        </div>
      )}
    </div>
  );
}
