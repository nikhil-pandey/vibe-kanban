import { useCallback, useMemo, useEffect } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Plus, FolderPlus } from 'lucide-react';
import { useAllTasks, useProjects } from '@/hooks';
import { ProjectFormDialog } from '@/components/dialogs/projects/ProjectFormDialog';
import { useSearch } from '@/contexts/SearchContext';
import { useTaskAttemptWithSession } from '@/hooks/useTaskAttempt';
import { useTaskAttempts } from '@/hooks/useTaskAttempts';
import { useBranchStatus, useAttemptExecution } from '@/hooks';
import { useMediaQuery } from '@/hooks/useMediaQuery';
import { paths } from '@/lib/paths';
import {
  DashboardFilters,
  DEFAULT_STATUS_FILTERS,
  ProjectSection,
} from '@/components/dashboard';
import { Loader } from '@/components/ui/loader';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { NewCard, NewCardHeader } from '@/components/ui/new-card';
import { TasksLayout, type LayoutMode } from '@/components/layout/TasksLayout';
import TaskPanel from '@/components/panels/TaskPanel';
import TaskAttemptPanel from '@/components/panels/TaskAttemptPanel';
import { PreviewPanel } from '@/components/panels/PreviewPanel';
import { DiffsPanel } from '@/components/panels/DiffsPanel';
import TodoPanel from '@/components/tasks/TodoPanel';
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbLink,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb';
import { AttemptHeaderActions } from '@/components/panels/AttemptHeaderActions';
import { TaskPanelHeaderActions } from '@/components/panels/TaskPanelHeaderActions';
import { ExecutionProcessesProvider } from '@/contexts/ExecutionProcessesContext';
import { ClickedElementsProvider } from '@/contexts/ClickedElementsProvider';
import { ReviewProvider } from '@/contexts/ReviewProvider';
import { GitOperationsProvider } from '@/contexts/GitOperationsContext';
import { ProjectProvider } from '@/contexts/ProjectContext';
import type {
  TaskStatus,
  TaskWithAttemptStatusAndProject,
  TaskWithAttemptStatus,
  RepoBranchStatus,
  Workspace,
} from 'shared/types';

function DiffsPanelContainer({
  attempt,
  selectedTask,
  branchStatus,
}: {
  attempt: Workspace | null;
  selectedTask: TaskWithAttemptStatus | null;
  branchStatus: RepoBranchStatus[] | null;
}) {
  const { isAttemptRunning } = useAttemptExecution(attempt?.id);

  return (
    <DiffsPanel
      key={attempt?.id}
      selectedAttempt={attempt}
      gitOps={
        attempt && selectedTask
          ? {
              task: selectedTask,
              branchStatus: branchStatus ?? null,
              isAttemptRunning,
              selectedBranch: branchStatus?.[0]?.target_branch_name ?? null,
            }
          : undefined
      }
    />
  );
}

export function Dashboard() {
  const { t } = useTranslation(['tasks', 'common']);
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const isXL = useMediaQuery('(min-width: 1280px)');
  const isMobile = !isXL;

  // Get task and attempt IDs from URL params
  const selectedTaskId = searchParams.get('task');
  const selectedProjectId = searchParams.get('project');
  const attemptId = searchParams.get('attempt');

  // Status filters from URL or defaults
  const statusFiltersParam = searchParams.get('status');
  const statusFilters: TaskStatus[] = useMemo(() => {
    if (!statusFiltersParam) return DEFAULT_STATUS_FILTERS;
    return statusFiltersParam.split(',') as TaskStatus[];
  }, [statusFiltersParam]);

  // Search
  const { query: searchQuery } = useSearch();
  const hasSearch = Boolean(searchQuery.trim());
  const normalizedSearch = searchQuery.trim().toLowerCase();

  // Fetch all tasks
  const {
    tasks,
    tasksById,
    tasksByProject,
    projectNames: taskProjectNames,
    isLoading: isTasksLoading,
    error: streamError,
  } = useAllTasks();

  // Fetch all projects (including those without tasks)
  const { projects: allProjects, projectsById, isLoading: isProjectsLoading } = useProjects();

  const isLoading = isTasksLoading || isProjectsLoading;

  // Merge project names from tasks with all projects to include empty projects
  const projectNames = useMemo(() => {
    const projectMap = new Map<string, string>();

    // Add all projects from the projects list
    allProjects.forEach((project) => {
      projectMap.set(project.id, project.name);
    });

    // Also add any from tasks (in case of timing issues)
    taskProjectNames.forEach((p) => {
      if (!projectMap.has(p.id)) {
        projectMap.set(p.id, p.name);
      }
    });

    return Array.from(projectMap.entries())
      .map(([id, name]) => ({ id, name }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [allProjects, taskProjectNames]);

  // Selected task from the tasksById map
  const selectedTask = useMemo(() => {
    if (!selectedTaskId) return null;
    return tasksById[selectedTaskId] ?? null;
  }, [selectedTaskId, tasksById]);

  // Convert to TaskWithAttemptStatus for panels (strip project_name)
  const selectedTaskForPanel: TaskWithAttemptStatus | null = useMemo(() => {
    if (!selectedTask) return null;
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const { project_name, ...rest } = selectedTask;
    return rest as TaskWithAttemptStatus;
  }, [selectedTask]);

  // Handle latest attempt redirect
  const isLatest = attemptId === 'latest';
  const { data: attempts = [], isLoading: isAttemptsLoading } = useTaskAttempts(
    selectedTaskId ?? undefined,
    { enabled: !!selectedTaskId && isLatest }
  );

  const latestAttemptId = useMemo(() => {
    if (!attempts?.length) return undefined;
    return [...attempts].sort((a, b) => {
      const diff =
        new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      if (diff !== 0) return diff;
      return a.id.localeCompare(b.id);
    })[0].id;
  }, [attempts]);

  useEffect(() => {
    if (!selectedTaskId || !selectedProjectId) return;
    if (!isLatest) return;
    if (isAttemptsLoading) return;

    const params = new URLSearchParams(searchParams);
    if (!latestAttemptId) {
      params.delete('attempt');
    } else {
      params.set('attempt', latestAttemptId);
    }
    setSearchParams(params, { replace: true });
  }, [selectedTaskId, selectedProjectId, isLatest, isAttemptsLoading, latestAttemptId, searchParams, setSearchParams]);

  const effectiveAttemptId = attemptId === 'latest' ? undefined : attemptId ?? undefined;
  const isTaskView = !!selectedTaskId && !effectiveAttemptId;
  const { data: attempt } = useTaskAttemptWithSession(effectiveAttemptId);
  const { data: branchStatus } = useBranchStatus(attempt?.id);

  // View mode
  const rawMode = searchParams.get('view') as LayoutMode;
  const mode: LayoutMode =
    rawMode === 'preview' || rawMode === 'diffs' ? rawMode : null;

  const setMode = useCallback(
    (newMode: LayoutMode) => {
      const params = new URLSearchParams(searchParams);
      if (newMode === null) {
        params.delete('view');
      } else {
        params.set('view', newMode);
      }
      setSearchParams(params, { replace: true });
    },
    [searchParams, setSearchParams]
  );

  const handleStatusFilterChange = useCallback(
    (newStatuses: TaskStatus[]) => {
      const params = new URLSearchParams(searchParams);
      const isDefault =
        newStatuses.length === DEFAULT_STATUS_FILTERS.length &&
        DEFAULT_STATUS_FILTERS.every((s) => newStatuses.includes(s));

      if (isDefault) {
        params.delete('status');
      } else {
        params.set('status', newStatuses.join(','));
      }
      setSearchParams(params, { replace: true });
    },
    [searchParams, setSearchParams]
  );

  const handleSelectTask = useCallback(
    (task: TaskWithAttemptStatusAndProject) => {
      const params = new URLSearchParams(searchParams);
      params.set('task', task.id);
      params.set('project', task.project_id);
      params.set('attempt', 'latest');
      params.delete('view');
      setSearchParams(params, { replace: true });
    },
    [searchParams, setSearchParams]
  );

  const handleClosePanel = useCallback(() => {
    const params = new URLSearchParams(searchParams);
    params.delete('task');
    params.delete('project');
    params.delete('attempt');
    params.delete('view');
    setSearchParams(params, { replace: true });
  }, [searchParams, setSearchParams]);

  const navigateToTaskView = useCallback(() => {
    if (!selectedTaskId || !selectedProjectId) return;
    const params = new URLSearchParams(searchParams);
    params.delete('attempt');
    params.delete('view');
    setSearchParams(params, { replace: true });
  }, [selectedTaskId, selectedProjectId, searchParams, setSearchParams]);

  // Filter tasks by status and search
  const filteredTasksByProject = useMemo(() => {
    const result: Record<string, TaskWithAttemptStatusAndProject[]> = {};

    Object.entries(tasksByProject).forEach(([projectId, projectTasks]) => {
      const filtered = projectTasks.filter((task) => {
        // Status filter
        if (!statusFilters.includes(task.status)) return false;

        // Search filter
        if (hasSearch) {
          const titleMatch = task.title.toLowerCase().includes(normalizedSearch);
          const descMatch = task.description?.toLowerCase().includes(normalizedSearch);
          if (!titleMatch && !descMatch) return false;
        }

        return true;
      });

      if (filtered.length > 0) {
        result[projectId] = filtered;
      }
    });

    return result;
  }, [tasksByProject, statusFilters, hasSearch, normalizedSearch]);

  // Get projects to display - show all projects, but only show projects with matching tasks when searching
  const visibleProjects = useMemo(() => {
    if (hasSearch) {
      // When searching, only show projects that have matching tasks
      return projectNames.filter((p) => filteredTasksByProject[p.id]?.length > 0);
    }
    // Otherwise show all projects (including empty ones)
    return projectNames;
  }, [projectNames, filteredTasksByProject, hasSearch]);

  const isPanelOpen = Boolean(selectedTask);

  const truncateTitle = (title: string | undefined, maxLength = 20) => {
    if (!title) return 'Task';
    if (title.length <= maxLength) return title;
    const truncated = title.substring(0, maxLength);
    const lastSpace = truncated.lastIndexOf(' ');
    return lastSpace > 0
      ? `${truncated.substring(0, lastSpace)}...`
      : `${truncated}...`;
  };

  const handleCreateProject = useCallback(async () => {
    await ProjectFormDialog.show({});
  }, []);

  if (isLoading && tasks.length === 0) {
    return <Loader message={t('loading', { defaultValue: 'Loading...' })} size={32} className="py-8" />;
  }

  // Dashboard content (left side)
  const dashboardContent = (
    <div className="flex flex-col h-full">
      {/* Filters and Actions */}
      <div className="px-4 py-3 border-b bg-background shrink-0">
        <div className="flex items-center justify-between gap-4 flex-wrap">
          <DashboardFilters
            selectedStatuses={statusFilters}
            onStatusChange={handleStatusFilterChange}
          />
          <Button
            onClick={handleCreateProject}
            size="sm"
            className="gap-2"
          >
            <FolderPlus className="h-4 w-4" />
            {t('projects.create', { defaultValue: 'New Project' })}
          </Button>
        </div>
      </div>

      {/* Project sections */}
      <div className="flex-1 overflow-y-auto p-4">
        {visibleProjects.length === 0 ? (
          <div className="text-center py-8">
            <p className="text-muted-foreground">
              {tasks.length === 0
                ? t('empty.noTasks', { defaultValue: 'No tasks found' })
                : t('empty.noSearchResults', { defaultValue: 'No matching tasks' })}
            </p>
            {tasks.length === 0 && (
              <Button
                className="mt-4"
                onClick={() => navigate(paths.projects())}
              >
                <Plus className="h-4 w-4 mr-2" />
                {t('empty.createFirst', { defaultValue: 'Create a project' })}
              </Button>
            )}
          </div>
        ) : (
          visibleProjects.map((project) => (
            <ProjectSection
              key={project.id}
              projectId={project.id}
              projectName={project.name}
              project={projectsById[project.id]}
              tasks={filteredTasksByProject[project.id] || []}
              selectedTaskId={selectedTaskId ?? undefined}
              onSelectTask={handleSelectTask}
              defaultExpanded={true}
            />
          ))
        )}
      </div>
    </div>
  );

  // Right panel header
  const rightHeader = selectedTask ? (
    <NewCardHeader
      className="shrink-0"
      actions={
        isTaskView ? (
          <TaskPanelHeaderActions
            task={selectedTaskForPanel!}
            onClose={handleClosePanel}
          />
        ) : (
          <AttemptHeaderActions
            mode={mode}
            onModeChange={setMode}
            task={selectedTaskForPanel!}
            attempt={attempt ?? null}
            onClose={handleClosePanel}
          />
        )
      }
    >
      <div className="mx-auto w-full">
        <Breadcrumb>
          <BreadcrumbList>
            <BreadcrumbItem>
              <BreadcrumbLink
                className="cursor-pointer hover:underline text-muted-foreground"
                onClick={() => navigate(paths.projectTasks(selectedTask.project_id))}
              >
                {selectedTask.project_name}
              </BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
            <BreadcrumbItem>
              {isTaskView ? (
                <BreadcrumbPage>{truncateTitle(selectedTask?.title)}</BreadcrumbPage>
              ) : (
                <BreadcrumbLink
                  className="cursor-pointer hover:underline"
                  onClick={navigateToTaskView}
                >
                  {truncateTitle(selectedTask?.title)}
                </BreadcrumbLink>
              )}
            </BreadcrumbItem>
            {!isTaskView && (
              <>
                <BreadcrumbSeparator />
                <BreadcrumbItem>
                  <BreadcrumbPage>{attempt?.branch || 'Task Attempt'}</BreadcrumbPage>
                </BreadcrumbItem>
              </>
            )}
          </BreadcrumbList>
        </Breadcrumb>
      </div>
    </NewCardHeader>
  ) : null;

  // Right panel content
  const attemptContent = selectedTask ? (
    <ProjectProvider projectId={selectedTask.project_id}>
      <NewCard className="h-full min-h-0 flex flex-col bg-diagonal-lines bg-muted border-0">
        {isTaskView ? (
          <TaskPanel task={selectedTaskForPanel} />
        ) : (
          <TaskAttemptPanel attempt={attempt} task={selectedTaskForPanel}>
            {({ logs, followUp }) => (
              <>
                <div className="flex-1 min-h-0 flex flex-col">
                  <div className="flex-1 min-h-0 flex flex-col">{logs}</div>

                  <div className="shrink-0 border-t">
                    <div className="mx-auto w-full max-w-[50rem]">
                      <TodoPanel />
                    </div>
                  </div>

                  <div className="min-h-0 max-h-[50%] border-t overflow-hidden bg-background">
                    <div className="mx-auto w-full max-w-[50rem] h-full min-h-0">
                      {followUp}
                    </div>
                  </div>
                </div>
              </>
            )}
          </TaskAttemptPanel>
        )}
      </NewCard>
    </ProjectProvider>
  ) : null;

  // Aux content (preview/diffs)
  const auxContent =
    selectedTask && attempt ? (
      <div className="relative h-full w-full">
        {mode === 'preview' && <PreviewPanel />}
        {mode === 'diffs' && (
          <DiffsPanelContainer
            attempt={attempt}
            selectedTask={selectedTaskForPanel}
            branchStatus={branchStatus ?? null}
          />
        )}
      </div>
    ) : (
      <div className="relative h-full w-full" />
    );

  const layoutContent = (
    <GitOperationsProvider attemptId={attempt?.id}>
      <ClickedElementsProvider attempt={attempt}>
        <ReviewProvider attemptId={attempt?.id}>
          <ExecutionProcessesProvider attemptId={attempt?.id}>
            <TasksLayout
              kanban={dashboardContent}
              attempt={attemptContent}
              aux={auxContent}
              isPanelOpen={isPanelOpen}
              mode={mode}
              isMobile={isMobile}
              rightHeader={rightHeader}
            />
          </ExecutionProcessesProvider>
        </ReviewProvider>
      </ClickedElementsProvider>
    </GitOperationsProvider>
  );

  return (
    <div className="min-h-full h-full flex flex-col">
      {streamError && (
        <Alert className="w-full z-30 xl:sticky xl:top-0">
          <AlertTitle className="flex items-center gap-2">
            <AlertTriangle size="16" />
            {t('common:states.reconnecting', { defaultValue: 'Reconnecting...' })}
          </AlertTitle>
          <AlertDescription>{streamError}</AlertDescription>
        </Alert>
      )}

      <div className="flex-1 min-h-0">{layoutContent}</div>
    </div>
  );
}
