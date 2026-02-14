import CodeMirror from '@uiw/react-codemirror';
import { json } from '@codemirror/lang-json';
import { EditorView } from '@codemirror/view';

type Props = {
  content: string;
  lang: string | null;
  theme?: 'light' | 'dark';
};

// Custom light theme to ensure readable text color
const lightTheme = EditorView.theme({
  '&': {
    backgroundColor: '#fff',
    color: '#24292e',
  },
  '.cm-content': {
    caretColor: '#24292e',
  },
});

// Map file extensions to CodeMirror language extensions
function getLanguageExtension(lang: string | null) {
  if (!lang) return [];
  const normalizedLang = lang.toLowerCase();
  if (normalizedLang === 'json') {
    return [json()];
  }
  // For other languages, CodeMirror will use basic highlighting
  return [];
}

/**
 * View syntax highlighted file content using CodeMirror.
 */
function FileContentView({ content, lang, theme }: Props) {
  // Avoid SSR errors
  if (typeof window === 'undefined') return null;

  const isDark = theme !== 'light'; // Default to dark theme

  return (
    <div className="border mt-2 rounded-md overflow-hidden">
      <CodeMirror
        value={content}
        height="auto"
        theme={isDark ? 'dark' : 'light'}
        editable={false}
        basicSetup={{
          lineNumbers: true,
          foldGutter: true,
          highlightActiveLineGutter: false,
          highlightActiveLine: false,
        }}
        extensions={[
          ...getLanguageExtension(lang),
          EditorView.lineWrapping,
          ...(isDark ? [] : [lightTheme]),
        ]}
        style={{
          fontSize: '12px',
        }}
      />
    </div>
  );
}

export default FileContentView;
