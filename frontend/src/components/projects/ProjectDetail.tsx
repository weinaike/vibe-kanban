import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { useNavigateWithSearch } from '@/hooks';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { FilesPanel } from '@/components/panels/FilesPanel';
import { projectsApi } from '@/lib/api';
import { useProjects } from '@/hooks/useProjects';
import { useProjectRepos } from '@/hooks';
import {
  AlertCircle,
  ArrowLeft,
  CheckSquare,
  Edit,
  Loader2,
  Trash2,
} from 'lucide-react';

interface ProjectDetailProps {
  projectId: string;
  onBack: () => void;
}

export function ProjectDetail({ projectId, onBack }: ProjectDetailProps) {
  const { t } = useTranslation('projects');
  const navigate = useNavigateWithSearch();
  const { projectsById, isLoading, error: projectsError } = useProjects();
  const { data: repos } = useProjectRepos(projectId);
  const [deleteError, setDeleteError] = useState('');

  const project = projectsById[projectId] || null;

  // Get the path from the first repo (if any)
  const projectPath = repos && repos.length > 0 ? repos[0].path : undefined;

  const handleDelete = async () => {
    if (!project) return;
    if (
      !confirm(
        `Are you sure you want to delete "${project.name}"? This action cannot be undone.`
      )
    )
      return;

    try {
      await projectsApi.delete(projectId);
      onBack();
    } catch (error) {
      console.error('Failed to delete project:', error);
      // @ts-expect-error it is type ApiError
      setDeleteError(error.message || t('errors.deleteFailed'));
      setTimeout(() => setDeleteError(''), 5000);
    }
  };

  const handleEditClick = () => {
    navigate(`/settings/projects?projectId=${projectId}`);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        Loading project...
      </div>
    );
  }

  if ((!project && !isLoading) || projectsError) {
    const errorMsg = projectsError
      ? projectsError.message
      : t('projectNotFound');
    return (
      <div className="flex flex-col h-full">
        <div className="border-b bg-background">
          <div className="flex items-center px-6 py-4">
            <Button variant="ghost" size="sm" onClick={onBack}>
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Button>
          </div>
        </div>
        <div className="flex-1 flex items-center justify-center">
          <div className="text-center">
            <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-muted mb-4">
              <AlertCircle className="h-8 w-8 text-muted-foreground" />
            </div>
            <h3 className="text-lg font-semibold mb-2">Project not found</h3>
            <p className="text-sm text-muted-foreground mb-4">{errorMsg}</p>
            <Button onClick={onBack}>Back to Projects</Button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b bg-background">
        <div className="flex items-center justify-between px-6 py-4">
          <div className="flex items-center gap-4">
            <Button variant="ghost" size="sm" onClick={onBack}>
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Button>
            <div className="h-6 w-px bg-border" />
            <div>
              <h1 className="text-xl font-semibold">{project.name}</h1>
              <p className="text-xs text-muted-foreground">
                {repos && repos.length > 0 ? repos[0].path : 'No repository configured'}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => navigate(`/projects/${projectId}/tasks`)}>
              <CheckSquare className="mr-2 h-4 w-4" />
              Tasks
            </Button>
            <Button variant="outline" size="sm" onClick={handleEditClick}>
              <Edit className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleDelete}
              className="text-destructive hover:text-destructive hover:bg-destructive/10"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 overflow-hidden">
        <FilesPanel rootPath={projectPath} />
      </div>

      {/* Error Alert */}
      {deleteError && (
        <div className="absolute bottom-4 right-4 z-50 max-w-md">
          <Alert variant="destructive">
            <AlertCircle className="h-4 w-4" />
            <AlertDescription>{deleteError}</AlertDescription>
          </Alert>
        </div>
      )}
    </div>
  );
}
