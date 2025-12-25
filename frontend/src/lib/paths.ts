export const paths = {
  dashboard: () => '/',
  dashboardTask: (taskId: string, projectId: string) => `/?task=${taskId}&project=${projectId}`,
  projects: () => '/projects',
  projectTasks: (projectId: string) => `/projects/${projectId}/tasks`,
  task: (projectId: string, taskId: string) =>
    `/projects/${projectId}/tasks/${taskId}`,
  attempt: (projectId: string, taskId: string, attemptId: string) =>
    `/projects/${projectId}/tasks/${taskId}/attempts/${attemptId}`,
  attemptFull: (projectId: string, taskId: string, attemptId: string) =>
    `/projects/${projectId}/tasks/${taskId}/attempts/${attemptId}/full`,
};
