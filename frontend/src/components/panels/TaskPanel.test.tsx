import { render, screen, fireEvent } from '@testing-library/react';
import { vi } from 'vitest';
import TaskPanel from './TaskPanel';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, opts?: Record<string, unknown>) =>
      (opts?.defaultValue as string) || _key,
  }),
}));

vi.mock('@/hooks/useTaskAttempts', () => ({
  useTaskAttemptsWithSessions: () => ({
    data: [],
    isLoading: false,
    isError: false,
  }),
}));

vi.mock('@/hooks/useTaskAttempt', () => ({
  useTaskAttemptWithSession: () => ({
    data: null,
    isLoading: false,
  }),
}));

vi.mock('@/hooks', () => ({
  useNavigateWithSearch: () => vi.fn(),
}));

vi.mock('@/contexts/ProjectContext', () => ({
  useProject: () => ({
    projectId: 'project-123',
    project: undefined,
    isLoading: false,
    error: null,
    isError: false,
  }),
}));

vi.mock('@/components/dialogs/tasks/CreateAttemptDialog', () => {
  const show = vi.fn();
  return {
    __esModule: true,
    CreateAttemptDialog: { show },
  };
});

vi.mock('@/components/ui/wysiwyg', () => ({
  __esModule: true,
  default: () => <div data-testid="wysiwyg" />,
}));

vi.mock('@/components/ui/new-card', () => ({
  NewCardContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));

vi.mock('@/components/ui/button', () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    variant?: string;
  }) => (
    <button type="button" {...props}>
      {children}
    </button>
  ),
}));

vi.mock('@/components/tasks/TodoPanel', () => ({
  __esModule: true,
  default: () => <div data-testid="todo-panel" />,
}));

vi.mock('@/components/ui/table', () => ({
  DataTable: ({
    headerContent,
  }: {
    headerContent: React.ReactNode;
  }) => <div data-testid="attempts-header">{headerContent}</div>,
}));

describe('TaskPanel', () => {
  it('passes projectId to CreateAttemptDialog when starting a new attempt', () => {
    const task = {
      id: 'task-1',
      title: 'Test task',
      description: '',
      parent_workspace_id: null,
    } as unknown as import('shared/types').TaskWithAttemptStatus;

    render(<TaskPanel task={task} />);

    const header = screen.getByTestId('attempts-header');
    const startButton = header.querySelector('button');
    expect(startButton).toBeInTheDocument();

    fireEvent.click(startButton!);

    expect(
      vi.mocked(CreateAttemptDialog.show)
    ).toHaveBeenCalledWith({
      taskId: 'task-1',
      projectId: 'project-123',
    });
  });
});
