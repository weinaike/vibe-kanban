import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Play,
  Edit3,
  Square,
  SquareTerminal,
  Save,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { ExecutionProcess, Project } from 'shared/types';
import {
  createScriptPlaceholderStrategy,
  ScriptPlaceholderContext,
} from '@/utils/scriptPlaceholders';
import { useUserSystem } from '@/components/ConfigProvider';
import { useProjectMutations } from '@/hooks/useProjectMutations';

interface NoServerContentProps {
  projectHasDevScript: boolean;
  runningDevServer: ExecutionProcess | undefined;
  isStartingDevServer: boolean;
  startDevServer: () => void;
  stopDevServer: () => void;
  project: Project | undefined;
}

export function NoServerContent({
  projectHasDevScript,
  runningDevServer,
  isStartingDevServer,
  startDevServer,
  stopDevServer,
  project,
}: NoServerContentProps) {
  const { t } = useTranslation('tasks');
  const [devScriptInput, setDevScriptInput] = useState('');
  const [saveError, setSaveError] = useState<string | null>(null);
  const [isEditingExistingScript, setIsEditingExistingScript] = useState(false);
  const { system } = useUserSystem();

  const { updateProject } = useProjectMutations();

  // Create strategy-based placeholders
  const placeholders = system.environment
    ? new ScriptPlaceholderContext(
        createScriptPlaceholderStrategy(system.environment.os_type)
      ).getPlaceholders()
    : {
        setup: '#!/bin/bash\nnpm install\n# Add any setup commands here...',
        dev: '#!/bin/bash\nnpm run dev\n# Add dev server start command here...',
        cleanup:
          '#!/bin/bash\n# Add cleanup commands here...\n# This runs after coding agent execution',
      };

  const handleSaveDevScript = async (startAfterSave?: boolean) => {
    setSaveError(null);
    if (!project) {
      setSaveError(t('preview.devScript.errors.notLoaded'));
      return;
    }

    const script = devScriptInput.trim();
    if (!script) {
      setSaveError(t('preview.devScript.errors.empty'));
      return;
    }

    updateProject.mutate(
      {
        projectId: project.id,
        data: {
          name: null,
          dev_script: script,
          dev_script_working_dir: project.dev_script_working_dir ?? null,
          default_agent_working_dir: project.default_agent_working_dir ?? null,
        },
      },
      {
        onSuccess: () => {
          setIsEditingExistingScript(false);
          if (startAfterSave) {
            startDevServer();
          }
        },
        onError: (err) => {
          setSaveError((err as Error)?.message || 'Failed to save dev script');
        },
      }
    );
  };

  const handleEditExistingScript = () => {
    if (project?.dev_script) {
      setDevScriptInput(project.dev_script);
    }
    setIsEditingExistingScript(true);
    setSaveError(null);
  };

  const handleCancelEdit = () => {
    setIsEditingExistingScript(false);
    setDevScriptInput('');
    setSaveError(null);
  };

  return (
    <div className="flex-1 flex items-center justify-center">
      <div className="text-center space-y-6 max-w-md mx-auto p-6">
        <div className="flex items-center justify-center">
          <SquareTerminal className="h-8 w-8 text-muted-foreground" />
        </div>

        <div className="space-y-4">
          <div>
            <h3 className="text-lg font-medium text-foreground mb-2">
              {t('preview.noServer.title')}
            </h3>
            <p className="text-sm text-muted-foreground">
              {projectHasDevScript
                ? t('preview.noServer.startPrompt')
                : t('preview.noServer.setupPrompt')}
            </p>
          </div>

          {!isEditingExistingScript ? (
            <div className="flex items-center justify-center gap-2">
              <Button
                variant={runningDevServer ? 'destructive' : 'default'}
                size="sm"
                onClick={() => {
                  if (runningDevServer) {
                    stopDevServer();
                  } else {
                    startDevServer();
                  }
                }}
                disabled={isStartingDevServer || !projectHasDevScript}
                className="gap-1"
              >
                {runningDevServer ? (
                  <>
                    <Square className="h-4 w-4" />
                    {t('preview.toolbar.stopDevServer')}
                  </>
                ) : (
                  <>
                    <Play className="h-4 w-4" />
                    {t('preview.noServer.startButton')}
                  </>
                )}
              </Button>

              {!runningDevServer && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleEditExistingScript}
                  className="gap-1"
                >
                  <Edit3 className="h-3 w-3" />
                  {t('preview.noServer.editButton')}
                </Button>
              )}
            </div>
          ) : (
            <div className="text-left">
              <div className="space-y-4">
                <Textarea
                  id="devScript"
                  placeholder={placeholders.dev}
                  value={devScriptInput}
                  onChange={(e) => setDevScriptInput(e.target.value)}
                  className="min-h-[120px] font-mono text-sm"
                  disabled={updateProject.isPending}
                />

                {saveError && (
                  <Alert variant="destructive">
                    <AlertDescription>{saveError}</AlertDescription>
                  </Alert>
                )}

                <div className="flex gap-2 justify-center">
                  {isEditingExistingScript ? (
                    <>
                      <Button
                        size="sm"
                        onClick={() => handleSaveDevScript(false)}
                        disabled={updateProject.isPending}
                        className="gap-1"
                      >
                        <Save className="h-3 w-3" />
                        {t('preview.devScript.saveChanges')}
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={handleCancelEdit}
                        disabled={updateProject.isPending}
                        className="gap-1"
                      >
                        <X className="h-3 w-3" />
                        {t('preview.devScript.cancel')}
                      </Button>
                    </>
                  ) : (
                    <>
                      <Button
                        size="sm"
                        onClick={() => handleSaveDevScript(true)}
                        disabled={updateProject.isPending}
                        className="gap-1"
                      >
                        <Play className="h-4 w-4" />
                        {t('preview.devScript.saveAndStart')}
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleSaveDevScript(false)}
                        disabled={updateProject.isPending}
                        className="gap-1"
                      >
                        <Save className="h-3 w-3" />
                        {t('preview.devScript.saveOnly')}
                      </Button>
                    </>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
