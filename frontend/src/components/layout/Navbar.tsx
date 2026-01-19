import { Link } from 'react-router-dom';
import { useCallback } from 'react';
import { Button } from '@/components/ui/button';
import {
  FolderOpen,
  Settings,
  BookOpen,
  MessageCircleQuestion,
  Plus,
  LogIn,
  LogOut,
  User,
} from 'lucide-react';
import { Logo } from '@/components/Logo';
import { SearchBar } from '@/components/SearchBar';
import { useSearch } from '@/contexts/SearchContext';
import { openTaskForm } from '@/lib/openTaskForm';
import { useProject } from '@/contexts/ProjectContext';
import { useOpenProjectInEditor } from '@/hooks/useOpenProjectInEditor';
import { OpenInIdeButton } from '@/components/ide/OpenInIdeButton';
import { useProjectRepos } from '@/hooks';
import { useAuth } from '@/hooks/auth/useAuth';
import { isLocalOrLanAccess } from '@/lib/accessControl';

const INTERNAL_NAV = [
  { label: 'Projects', icon: FolderOpen, to: '/projects' },
];

const SETTINGS_NAV = { label: 'Settings', icon: Settings, to: '/settings' };

const EXTERNAL_LINKS = [
  {
    label: 'Docs',
    icon: BookOpen,
    href: 'https://vibekanban.com/docs',
  },
  {
    label: 'Support',
    icon: MessageCircleQuestion,
    href: 'https://github.com/BloopAI/vibe-kanban/issues',
  },
];

function NavDivider() {
  return (
    <div
      className="mx-2 h-6 w-px bg-border/60"
      role="separator"
      aria-orientation="vertical"
    />
  );
}

export function Navbar() {
  const { projectId, project } = useProject();
  const { query, setQuery, active, clear, registerInputRef } = useSearch();
  const handleOpenInEditor = useOpenProjectInEditor(project || null);
  const { isSignedIn, user, login, logout } = useAuth();

  const { data: repos } = useProjectRepos(projectId);
  const isSingleRepoProject = repos?.length === 1;

  const setSearchBarRef = useCallback(
    (node: HTMLInputElement | null) => {
      registerInputRef(node);
    },
    [registerInputRef]
  );

  const handleCreateTask = () => {
    if (projectId) {
      openTaskForm({ mode: 'create', projectId });
    }
  };

  const handleOpenInIDE = () => {
    handleOpenInEditor();
  };

  return (
    <div className="border-b bg-background">
      <div className="w-full px-3">
        <div className="flex items-center h-12 py-2">
          <div className="flex-1 flex items-center">
            <Link to="/projects">
              <Logo />
            </Link>
          </div>

          <div className="hidden sm:flex items-center gap-2">
            <SearchBar
              ref={setSearchBarRef}
              className="shrink-0"
              value={query}
              onChange={setQuery}
              disabled={!active}
              onClear={clear}
              project={project || null}
            />
          </div>

          <div className="flex flex-1 items-center justify-end gap-1">
            {projectId ? (
              <>
                <div className="flex items-center gap-1">
                  {isSingleRepoProject && (
                    <OpenInIdeButton
                      onClick={handleOpenInIDE}
                      className="h-9 w-9"
                    />
                  )}
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-9 w-9"
                    onClick={handleCreateTask}
                    aria-label="Create new task"
                  >
                    <Plus className="h-4 w-4" />
                  </Button>
                </div>
                <NavDivider />
              </>
            ) : null}

            <div className="flex items-center gap-1">
              {INTERNAL_NAV.map((item) => {
                const active = location.pathname.startsWith(item.to);
                const Icon = item.icon;
                return (
                  <Button
                    key={item.to}
                    variant="ghost"
                    className={`[&_a]:flex [&_a]:items-center [&_a]:justify-center [&_a]:gap-2 h-9 w-9 sm:h-auto sm:w-auto sm:[&]:px-3 sm:[&]:py-1.5 ${active ? 'bg-accent' : ''}`}
                    asChild
                    aria-label={item.label}
                  >
                    <Link to={item.to} className="sm:flex sm:items-center sm:justify-center sm:gap-2">
                      <Icon className="h-4 w-4 shrink-0" />
                      <span className="hidden sm:inline">{item.label}</span>
                    </Link>
                  </Button>
                );
              })}

              {/* External links (Docs, Support) - hidden on mobile */}
              {EXTERNAL_LINKS.map((item) => {
                const Icon = item.icon;
                return (
                  <Button
                    key={item.href}
                    variant="ghost"
                    className="hidden sm:[&_a]:flex [&_a]:items-center [&_a]:justify-center [&_a]:gap-2 h-9 w-9 sm:h-auto sm:w-auto sm:[&]:px-3 sm:[&]:py-1.5"
                    asChild
                    aria-label={item.label}
                  >
                    <a
                      href={item.href}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="sm:flex sm:items-center sm:justify-center sm:gap-2 hidden sm:flex"
                    >
                      <Icon className="h-4 w-4 shrink-0" />
                      <span className="hidden sm:inline">{item.label}</span>
                    </a>
                  </Button>
                );
              })}

              <Button
                variant="ghost"
                className="[&_a]:flex [&_a]:items-center [&_a]:justify-center [&_a]:gap-2 h-9 w-9 sm:h-auto sm:w-auto sm:[&]:px-3 sm:[&]:py-1.5"
                asChild
                aria-label={SETTINGS_NAV.label}
              >
                <Link
                  to={
                    projectId
                      ? `/settings/projects?projectId=${projectId}`
                      : SETTINGS_NAV.to
                  }
                  className="sm:flex sm:items-center sm:justify-center sm:gap-2"
                >
                  <Settings className="h-4 w-4 shrink-0" />
                  <span className="hidden sm:inline">{SETTINGS_NAV.label}</span>
                </Link>
              </Button>

              {/* Auth Button */}
              {isSignedIn ? (
                <Button
                  variant="ghost"
                  className="flex items-center justify-center gap-2 h-9 w-9 sm:h-auto sm:w-auto sm:px-3 sm:py-1.5"
                  onClick={logout}
                  aria-label="Logout"
                >
                  <User className="h-4 w-4 shrink-0" />
                  <span className="hidden sm:inline">
                    {user?.display_name || user?.name}
                  </span>
                  <LogOut className="h-4 w-4 shrink-0 hidden sm:inline" />
                </Button>
              ) : isLocalOrLanAccess() ? (
                <Button
                  variant="ghost"
                  className="flex items-center justify-center gap-2 h-9 w-9 sm:h-auto sm:w-auto sm:px-3 sm:py-1.5"
                  onClick={login}
                  aria-label="Login"
                >
                  <LogIn className="h-4 w-4 shrink-0" />
                  <span className="hidden sm:inline">Login</span>
                </Button>
              ) : null}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
