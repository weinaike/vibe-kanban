import { useState, useEffect, useRef, useCallback } from 'react';
import { Loader2, File, Folder, FolderOpen, ChevronRight, ArrowLeft } from 'lucide-react';
import { fileSystemApi } from '@/lib/api';
import { getHighLightLanguageFromPath } from '@/utils/extToLanguage';
import FileContentView from '@/components/NormalizedConversation/FileContentView';
import { NewCardHeader } from '@/components/ui/new-card';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { DirectoryEntry } from 'shared/types';

interface FileNode {
  name: string;
  path: string;
  isDirectory: boolean;
  children?: FileNode[];
  loaded?: boolean;
}

interface FilesPanelProps {
  rootPath?: string;
  onFileSelect?: (path: string) => void;
}

export function FilesPanel({ rootPath, onFileSelect }: FilesPanelProps) {
  const [currentPath, setCurrentPath] = useState<string>(rootPath || '');
  const [directoryTree, setDirectoryTree] = useState<FileNode | null>(null);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [viewMode, setViewMode] = useState<'tree' | 'file'>('tree');
  const loadedPathsRef = useRef<Set<string>>(new Set());

  const buildTree = (entries: DirectoryEntry[]): FileNode[] => {
    return entries
      .filter((entry) => !entry.name.startsWith('.'))
      .map((entry) => ({
        name: entry.name,
        path: entry.path.toString(),
        isDirectory: entry.is_directory,
        children: entry.is_directory ? [] : undefined,
        loaded: false,
      }))
      .sort((a, b) => {
        if (a.isDirectory && !b.isDirectory) return -1;
        if (!a.isDirectory && b.isDirectory) return 1;
        return a.name.localeCompare(b.name);
      });
  };

  const loadDirectory = async (path: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await fileSystemApi.list(path);
      const nodes = buildTree(response.entries);
      setDirectoryTree({ name: '.', path, isDirectory: true, children: nodes, loaded: true });
      setCurrentPath(response.current_path);
      loadedPathsRef.current.add(path);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load directory');
    } finally {
      setIsLoading(false);
    }
  };

  const loadFile = async (path: string) => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await fileSystemApi.readFile(path);
      setFileContent(response.content);
      setSelectedFilePath(path);
      setViewMode('file');
      onFileSelect?.(path);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load file');
    } finally {
      setIsLoading(false);
    }
  };

  const loadSubdirectory = useCallback(async (node: FileNode): Promise<FileNode[]> => {
    if (loadedPathsRef.current.has(node.path)) {
      return [];
    }

    setLoadingPaths((prev) => new Set(prev).add(node.path));
    setError(null);
    try {
      const response = await fileSystemApi.list(node.path);
      const nodes = buildTree(response.entries);
      loadedPathsRef.current.add(node.path);
      return nodes;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load directory');
      return [];
    } finally {
      setLoadingPaths((prev) => {
        const next = new Set(prev);
        next.delete(node.path);
        return next;
      });
    }
  }, []);

  const updateNodeChildren = useCallback((tree: FileNode | null, targetPath: string, newChildren: FileNode[]): FileNode | null => {
    if (!tree) return null;

    if (tree.path === targetPath) {
      return { ...tree, children: newChildren, loaded: true };
    }

    if (tree.children) {
      const updatedChildren = tree.children.map((child) =>
        updateNodeChildren(child, targetPath, newChildren)
      );
      return { ...tree, children: updatedChildren.filter(Boolean) as FileNode[] };
    }

    return tree;
  }, []);

  const toggleExpand = useCallback(async (node: FileNode) => {
    const isExpanded = expandedPaths.has(node.path);

    setExpandedPaths((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(node.path)) {
        newSet.delete(node.path);
      } else {
        newSet.add(node.path);
      }
      return newSet;
    });

    // Load subdirectory contents if expanding and not yet loaded
    if (!isExpanded && node.isDirectory && !node.loaded) {
      const children = await loadSubdirectory(node);
      if (children.length > 0) {
        setDirectoryTree((prev) => updateNodeChildren(prev, node.path, children));
      }
    }
  }, [expandedPaths, loadSubdirectory, updateNodeChildren]);

  const backToTree = () => {
    setViewMode('tree');
  };

  useEffect(() => {
    loadedPathsRef.current.clear();
    loadDirectory(currentPath);
    setViewMode('tree');
  }, [currentPath]);

  const TreeNode = ({ node, depth = 0 }: { node: FileNode; depth?: number }) => {
    const isExpanded = expandedPaths.has(node.path);
    const isLoading = loadingPaths.has(node.path);
    const Icon = node.isDirectory ? (isExpanded ? FolderOpen : Folder) : File;

    const handleClick = () => {
      if (node.isDirectory) {
        toggleExpand(node);
      } else {
        loadFile(node.path);
      }
    };

    return (
      <div>
        <div
          className={cn(
            'flex items-center gap-2 py-2 px-3 hover:bg-muted cursor-pointer rounded',
            selectedFilePath === node.path && 'bg-accent'
          )}
          style={{ paddingLeft: `${depth * 16 + 12}px` }}
          onClick={handleClick}
        >
          {node.isDirectory && (
            <>
              {isLoading ? (
                <Loader2 className="w-4 h-4 animate-spin shrink-0" />
              ) : (
                <ChevronRight
                  className={cn(
                    'w-4 h-4 transition-transform shrink-0',
                    isExpanded && 'transform rotate-90'
                  )}
                />
              )}
            </>
          )}
          <Icon className="w-4 h-4 shrink-0" />
          <span className="text-sm truncate">{node.name}</span>
        </div>
        {isExpanded && node.children && node.children.length > 0 && (
          <div>
            {node.children.map((child) => (
              <TreeNode key={child.path} node={child} depth={depth + 1} />
            ))}
          </div>
        )}
      </div>
    );
  };

  const language = selectedFilePath
    ? getHighLightLanguageFromPath(selectedFilePath)
    : null;

  return (
    <div className="h-full flex flex-col">
      <NewCardHeader
        className="sticky top-0 z-10 shrink-0"
        actions={
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              loadedPathsRef.current.clear();
              setExpandedPaths(new Set());
              setViewMode('tree');
              loadDirectory(currentPath);
            }}
            disabled={isLoading}
          >
            {isLoading && <Loader2 className="w-4 h-4 mr-2 animate-spin" />}
            Refresh
          </Button>
        }
      >
        <div className="text-sm text-muted-foreground truncate">
          {viewMode === 'file' ? selectedFilePath : (currentPath || 'File Browser')}
        </div>
      </NewCardHeader>

      {error && (
        <div className="px-3 py-2 bg-destructive/10 text-destructive text-sm">
          {error}
        </div>
      )}

      <div className="flex-1 flex overflow-hidden min-h-0">
        {/* Mobile: Show either tree or file, Desktop: Show both side by side */}
        {/* Directory Tree */}
        <div className={cn(
          "overflow-y-auto shrink-0",
          viewMode === 'tree' ? 'w-full' : 'hidden md:block md:w-64 md:border-r'
        )}>
          {isLoading && !directoryTree && (
            <div className="flex items-center justify-center h-full">
              <Loader2 className="w-6 h-6 animate-spin" />
            </div>
          )}
          {directoryTree?.children?.map((node) => (
            <TreeNode key={node.path} node={node} />
          ))}
          {directoryTree?.children && directoryTree.children.length === 0 && (
            <div className="flex items-center justify-center h-full text-sm text-muted-foreground">
              This directory is empty
            </div>
          )}
        </div>

        {/* File Content */}
        <div className={cn(
          "overflow-y-auto",
          viewMode === 'file' ? 'w-full' : 'hidden md:flex md:flex-1'
        )}>
          {fileContent ? (
            <div className="p-4">
              {/* Mobile back button */}
              <div className="md:hidden mb-3">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={backToTree}
                  className="w-full sm:w-auto"
                >
                  <ArrowLeft className="mr-2 h-4 w-4" />
                  Back to Files
                </Button>
              </div>
              <div className="hidden md:block text-sm text-muted-foreground mb-2 truncate">
                {selectedFilePath}
              </div>
              <FileContentView content={fileContent} lang={language} />
            </div>
          ) : (
            <div className="flex items-center justify-center h-full text-sm text-muted-foreground">
              Select a file to view
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
