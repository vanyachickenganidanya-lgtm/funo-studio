import './styles.css';
import * as monaco from 'monaco-editor';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import { registerFunoLanguage, setDiagnostics } from './funo-language';
import {
  ensureProject, saveFile, checkCode, transpileSource, runCode, buildMinecraft, fetchRegistry, installPackage,
  fetchMinecraftVersions, createMinecraftProject, minecraftToolchainStatus, installMinecraftToolchain,
  createFolder, reloadProject, setPathHidden,
  loadSettings, saveSettings, getPathStatus, installPath, uninstallPath, runBackend,
  listInstances, createInstance, updateInstance, deleteInstance, launchInstance, openAndroidLauncher,
  searchModrinth, installModrinth, removeInstanceMod,
  createPlugin, checkPlugin, installPlugin, listPlugins,
  beginMicrosoftAuth, completeMicrosoftAuth, currentMicrosoftAccount, logoutMicrosoft,
  type Project, type Diagnostic, type RegistryPackage, type MinecraftVersion, type StudioSettings,
  type MinecraftInstance, type MinecraftToolchainStatus, type ModrinthProject, type PluginProject,
  type MinecraftAccount, runtimePlatform, desktopToolsAvailable
} from './api';

(self as any).MonacoEnvironment = { getWorker: () => new EditorWorker() };

const icon = (name: string) => {
  const paths: Record<string, string> = {
    files: '<path d="M4 4h6l2 2h8v14H4z"/><path d="M4 9h16"/>',
    search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
    package: '<path d="m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3Z"/><path d="m4.5 7.5 7.5 4 7.5-4M12 21v-9.5"/>',
    play: '<path d="M8 5v14l11-7Z"/>',
    book: '<path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H11v16H6.5A2.5 2.5 0 0 0 4 21.5zM20 5.5A2.5 2.5 0 0 0 17.5 3H13v16h4.5a2.5 2.5 0 0 1 2.5 2.5z"/>',
    cube: '<path d="m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3Z"/><path d="m4.5 7.5 7.5 4 7.5-4"/>',
    gear: '<circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-2-3.4-2.4 1A8 8 0 0 0 15 6l-.3-2.6h-4L10.4 6A8 8 0 0 0 9 7.1l-2.4-1-2 3.4 2 1.5a7 7 0 0 0 0 2l-2 1.5 2 3.4 2.4-1A8 8 0 0 0 10.4 18l.3 2.6h4L15 18a8 8 0 0 0 1.5-1.1l2.4 1 2-3.4-2-1.5a7 7 0 0 0 .1-1Z"/>',
    close: '<path d="m6 6 12 12M18 6 6 18"/>',
    check: '<path d="m5 12 4 4L19 6"/>',
    branch: '<circle cx="6" cy="5" r="2"/><circle cx="18" cy="6" r="2"/><circle cx="6" cy="19" r="2"/><path d="M6 7v10M8 10c5 0 8-1 8-3"/>',
    warning: '<path d="m12 3 10 18H2L12 3Z"/><path d="M12 9v5M12 18h.01"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    refresh: '<path d="M20 7v5h-5M4 17v-5h5"/><path d="M6 9a7 7 0 0 1 12-2l2 5M18 15a7 7 0 0 1-12 2l-2-5"/>',
    spark: '<path d="m12 3 1.2 4.5a4.5 4.5 0 0 0 3.3 3.3L21 12l-4.5 1.2a4.5 4.5 0 0 0-3.3 3.3L12 21l-1.2-4.5a4.5 4.5 0 0 0-3.3-3.3L3 12l4.5-1.2a4.5 4.5 0 0 0 3.3-3.3L12 3Z"/>',
    terminal: '<path d="m5 7 4 4-4 4M12 17h7"/>',
    java: '<path d="M8 19c-4-1-4-3 0-4M16 19c4-1 4-3 0-4M8 21h8M12 14c-4-3 5-4 1-8M11 3c5 2-3 4 2 7"/>'
  };
  return `<svg viewBox="0 0 24 24" aria-hidden="true">${paths[name] || paths.files}</svg>`;
};

const compactLayout = () => window.matchMedia('(max-width: 650px)').matches;
document.body.classList.toggle('android-app', runtimePlatform.android);
if (runtimePlatform.android) document.body.classList.add('panel-collapsed');

const app = document.querySelector<HTMLDivElement>('#app')!;
app.innerHTML = `
<div class="shell">
  <header class="titlebar" data-tauri-drag-region>
    <div class="app-mark">f;</div>
    <nav class="menu"><button>Файл</button><button>Правка</button><button>Выделение</button><button>Вид</button><button>Запуск</button><button>Помощь</button></nav>
    <button class="command-center" id="commandCenter">${icon('search')} <span>Мой первый проект</span><kbd>Ctrl K</kbd></button>
    <span class="android-chip">ANDROID</span>
    <div class="title-actions">
      <div class="mode-toggle"><button id="noviceMode" class="active">Новичок</button><button id="proMode">Профи</button></div>
    </div>
  </header>
  <div class="workbench">
    <nav class="activitybar">
      <div>
        <button class="activity active" data-view="explorer" title="Проводник">${icon('files')}</button>
        <button class="activity" data-view="search" title="Поиск">${icon('search')}</button>
        <button class="activity" data-view="packages" title="Библиотеки Funo Pack">${icon('package')}<span class="badge" id="packageBadge">•</span></button>
        <button class="activity" data-view="run" title="Запуск и отладка">${icon('play')}</button>
        <button class="activity" data-view="minecraft" title="Minecraft">${icon('cube')}</button>
        <button class="activity" data-view="wiki" title="Вики">${icon('book')}</button>
      </div>
      <button class="activity bottom" data-view="settings" title="Настройки">${icon('gear')}</button>
    </nav>
    <aside class="sidebar">
      <div class="sidebar-title"><span id="sidebarTitle">ПРОВОДНИК</span><div><button class="tiny" id="newFile" title="Новый файл">${icon('plus')}</button><button class="tiny" id="newFolder" title="Новая папка">▣</button><button class="tiny" id="refreshView">${icon('refresh')}</button><button class="tiny sidebar-close" id="sidebarClose" title="Закрыть">${icon('close')}</button></div></div>
      <div class="sidebar-content" id="sidebarContent"></div>
    </aside>
    <button class="drawer-scrim" id="drawerScrim" aria-label="Закрыть боковую панель"></button>
    <section class="editor-group">
      <div class="editor-tabs"><button class="editor-tab active"><span class="fun-icon">fn</span><span id="tabTitle">main.fun</span><span class="dirty" id="dirtyDot"></span>${icon('close')}</button><div class="editor-actions"><button id="markdownPreview" class="hidden" title="Предпросмотр Markdown">${icon('book')}</button><button id="showJava" title="Показать сгенерированный код">${icon('java')}</button><button id="topRun" title="Запустить Ctrl+Enter">${icon('play')}</button></div></div>
      <div class="breadcrumbs"><span id="projectCrumb">Мой первый проект</span><b>›</b><span id="fileCrumb">main.fun</span><b>›</b><span id="symbolCrumb">fun main()</span></div>
      <div class="editor-wrap">
        <div id="editor"></div>
        <article class="markdown-preview hidden" id="markdownPane"></article>
        <div class="friendly-card hidden" id="friendlyCard"></div>
        <div class="surface hidden" id="surface"></div>
      </div>
      <div class="panel">
        <div class="panel-head">
          <button class="panel-tab active" data-panel="terminal">ТЕРМИНАЛ</button>
          <button class="panel-tab" data-panel="problems">ПРОБЛЕМЫ <span id="problemCount">0</span></button>
          <button class="panel-tab" data-panel="output">ВЫХОД</button>
          <div class="panel-actions"><button id="collapsePanel" title="Свернуть или развернуть панель">⌃</button><button id="clearPanel" title="Очистить">${icon('close')}</button></div>
        </div>
        <pre class="panel-body" id="panelBody"><span class="muted">Funo готов. Нажмите Ctrl+Enter, чтобы запустить программу.</span></pre>
      </div>
    </section>
  </div>
  <footer class="statusbar">
    <span>${icon('branch')} main*</span><span id="syncStatus">${icon('check')} сохранено</span>
    <span id="errorStatus">× 0&nbsp;&nbsp; △ 0</span>
    <span class="status-spacer"></span><span id="cursorStatus">Стр 1, Стлб 1</span><span>Пробелы: 4</span><span>UTF-8</span><span id="languageStatus">{ } Funo</span><span id="backendStatus">JVM</span>
  </footer>
</div>
<div class="toast" id="toast"></div>
<div class="modal-layer hidden" id="modalLayer"><div class="modal" id="modal"></div></div>`;

registerFunoLanguage();
monaco.editor.defineTheme('funo-vscode', {
  base: 'vs-dark', inherit: true,
  rules: [
    { token: 'keyword.funo', foreground: 'C586C0' },
    { token: 'type.identifier.funo', foreground: '4EC9B0' },
    { token: 'predefined.funo', foreground: 'DCDCAA' },
    { token: 'string.funo', foreground: 'CE9178' },
    { token: 'number.funo', foreground: 'B5CEA8' },
    { token: 'comment.funo', foreground: '6A9955' }
  ],
  colors: {
    'editor.background': '#1f1f1f', 'editor.foreground': '#d4d4d4',
    'editorLineNumber.foreground': '#6e7681', 'editorLineNumber.activeForeground': '#c6c6c6',
    'editorCursor.foreground': '#aeafad', 'editor.selectionBackground': '#264f78',
    'editor.inactiveSelectionBackground': '#3a3d41', 'editorIndentGuide.background1': '#333333',
    'editorIndentGuide.activeBackground1': '#555555', 'editorBracketMatch.background': '#2b2b2b',
    'editorBracketMatch.border': '#888888', 'editorGutter.background': '#1f1f1f'
  }
});

const touchEditor = runtimePlatform.mobile || compactLayout();
const editor = monaco.editor.create(document.getElementById('editor')!, {
  value: '', language: 'funo', theme: 'funo-vscode', automaticLayout: true,
  fontFamily: '"Cascadia Code", "JetBrains Mono", Consolas, monospace', fontSize: touchEditor ? 15 : 14,
  lineHeight: touchEditor ? 24 : 22, fontLigatures: true, minimap: { enabled: !touchEditor, scale: 1 },
  smoothScrolling: true, cursorSmoothCaretAnimation: 'on', padding: { top: 10, bottom: touchEditor ? 28 : 0 },
  renderWhitespace: 'selection', bracketPairColorization: { enabled: true },
  guides: { bracketPairs: true, indentation: true }, stickyScroll: { enabled: !touchEditor },
  suggest: { showWords: false, preview: true }, quickSuggestions: { other: true, comments: false, strings: false },
  inlineSuggest: { enabled: true }, lightbulb: { enabled: monaco.editor.ShowLightbulbIconMode.OnCode },
  wordWrap: touchEditor ? 'on' : 'off', tabSize: 4, insertSpaces: true
});

let project: Project;
let currentPath = 'main.fun';
let diagnostics: Diagnostic[] = [];
let currentPanel: 'terminal' | 'problems' | 'output' = 'terminal';
let currentView = 'explorer';
let saveTimer = 0;
let checkTimer = 0;
let mode: 'novice' | 'pro' = (localStorage.getItem('funo-mode') as any) || 'novice';
let settings: StudioSettings;
let compilerBackend = 'jvm';
let markdownSplit = false;
let instances: MinecraftInstance[] = [];
let account: MinecraftAccount | null = null;
const models = new Map<string, monaco.editor.ITextModel>();

function toast(message: string, kind: 'ok' | 'warn' = 'ok') {
  const el = document.getElementById('toast')!;
  el.className = `toast show ${kind}`;
  el.innerHTML = `${kind === 'ok' ? icon('check') : icon('warning')}<span>${message}</span>`;
  window.setTimeout(() => el.classList.remove('show'), 3200);
}

function setMode(next: 'novice' | 'pro') {
  mode = next; localStorage.setItem('funo-mode', mode);
  document.body.classList.toggle('pro-mode', next === 'pro');
  document.getElementById('noviceMode')!.classList.toggle('active', next === 'novice');
  document.getElementById('proMode')!.classList.toggle('active', next === 'pro');
  editor.updateOptions({ minimap: { enabled: !touchEditor && next === 'pro' }, inlayHints: { enabled: next === 'pro' ? 'on' : 'off' } });
  if (settings) { settings.beginner = next === 'novice'; void saveSettings(settings); }
  if (diagnostics.length) renderFriendlyError(diagnostics[0]);
}

document.getElementById('noviceMode')!.onclick = () => setMode('novice');
document.getElementById('proMode')!.onclick = () => setMode('pro');

function manifestValue(source: string, section: string, key: string) {
  let currentSection = '';
  for (const rawLine of source.split(/\r?\n/)) {
    const line = rawLine.replace(/\s+#.*$/, '').trim();
    const heading = /^\[([^\]]+)]$/.exec(line);
    if (heading) { currentSection = heading[1].trim(); continue; }
    if (currentSection !== section) continue;
    const pair = /^([A-Za-z0-9_-]+)\s*=\s*"([^"]*)"/.exec(line);
    if (pair?.[1] === key) return pair[2];
  }
  return undefined;
}

function languageFor(path: string) {
  const extension = path.split('.').pop()?.toLowerCase() || '';
  const languages: Record<string, string> = {
    fun: 'funo', json: 'json', toml: 'ini', ini: 'ini', md: 'markdown', markdown: 'markdown',
    rs: 'rust', cpp: 'cpp', cc: 'cpp', cxx: 'cpp', c: 'c', h: 'cpp', hpp: 'cpp',
    java: 'java', kt: 'kotlin', kts: 'kotlin', py: 'python', js: 'javascript', mjs: 'javascript',
    jsx: 'javascript', ts: 'typescript', tsx: 'typescript', html: 'html', css: 'css', scss: 'scss',
    xml: 'xml', yaml: 'yaml', yml: 'yaml', sh: 'shell', ps1: 'powershell', bat: 'bat', cmd: 'bat',
    gradle: 'groovy', properties: 'ini'
  };
  return languages[extension] || 'plaintext';
}

function modelFor(path: string) {
  if (models.has(path)) return models.get(path)!;
  const file = project.files.find(f => f.path === path);
  const model = monaco.editor.createModel(file?.content || '', languageFor(path), monaco.Uri.parse(`funo://project/${path}`));
  models.set(path, model);
  return model;
}

function openFile(path: string) {
  currentPath = path;
  editor.setModel(modelFor(path));
  document.getElementById('tabTitle')!.textContent = path.split('/').pop() || path;
  document.getElementById('fileCrumb')!.textContent = path;
  const language = languageFor(path);
  document.getElementById('symbolCrumb')!.textContent = language === 'funo' ? 'Funo' : language;
  document.getElementById('languageStatus')!.textContent = `{ } ${language}`;
  const markdown = language === 'markdown';
  document.getElementById('markdownPreview')!.classList.toggle('hidden', !markdown);
  if (!markdown) { markdownSplit = false; document.getElementById('markdownPane')!.classList.add('hidden'); document.querySelector('.editor-wrap')!.classList.remove('markdown-split'); }
  document.getElementById('showJava')!.classList.toggle('hidden', language !== 'funo');
  document.getElementById('topRun')!.classList.toggle('hidden', language !== 'funo');
  document.querySelectorAll('.file-row').forEach(x => x.classList.toggle('active', (x as HTMLElement).dataset.path === path));
  hideSurface();
  updateMarkdownPreview();
  void diagnose();
}

function fileIcon(path: string) {
  const extension = path.split('.').pop()?.toLowerCase();
  if (extension === 'fun') return '<span class="file-type fun">fn</span>';
  if (extension === 'toml' || extension === 'json') return '<span class="file-type toml">⚙</span>';
  if (extension === 'md' || extension === 'markdown') return '<span class="file-type markdown">M↓</span>';
  if (extension === 'rs') return '<span class="file-type rust">Rs</span>';
  if (['cpp', 'cc', 'c', 'h', 'hpp'].includes(extension || '')) return '<span class="file-type cpp">C+</span>';
  return `<span class="file-type">${escapeHtml((extension || '·').slice(0, 2))}</span>`;
}

function markdownHtml(source: string) {
  let value = escapeHtml(source);
  value = value.replace(/^```([^\n]*)\n([\s\S]*?)^```$/gm, '<pre><code data-language="$1">$2</code></pre>');
  value = value.replace(/^### (.+)$/gm, '<h3>$1</h3>').replace(/^## (.+)$/gm, '<h2>$1</h2>').replace(/^# (.+)$/gm, '<h1>$1</h1>');
  value = value.replace(/^&gt; (.+)$/gm, '<blockquote>$1</blockquote>').replace(/^- (.+)$/gm, '<li>$1</li>');
  value = value.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>').replace(/`([^`]+)`/g, '<code>$1</code>');
  return value.split(/\n{2,}/).map(block => /^<(h\d|pre|blockquote|li)/.test(block) ? block : `<p>${block.replace(/\n/g, '<br>')}</p>`).join('');
}

function updateMarkdownPreview() {
  if (languageFor(currentPath) !== 'markdown') return;
  document.getElementById('markdownPane')!.innerHTML = markdownHtml(editor.getValue());
}

document.getElementById('markdownPreview')!.onclick = () => {
  markdownSplit = !markdownSplit;
  document.querySelector('.editor-wrap')!.classList.toggle('markdown-split', markdownSplit);
  document.getElementById('markdownPane')!.classList.toggle('hidden', !markdownSplit);
  updateMarkdownPreview();
};

function renderExplorer() {
  type Tree = { directories: Map<string, Tree>; files: typeof project.files };
  const root: Tree = { directories: new Map(), files: [] };
  const directoryPaths = new Set(project.directories || []);
  project.files.forEach(file => {
    const pieces = file.path.split('/'); pieces.pop();
    let current = '';
    pieces.forEach(piece => { current = current ? `${current}/${piece}` : piece; directoryPaths.add(current); });
  });
  [...directoryPaths].sort().forEach(path => {
    let node = root;
    path.split('/').filter(Boolean).forEach(piece => {
      if (!node.directories.has(piece)) node.directories.set(piece, { directories: new Map(), files: [] });
      node = node.directories.get(piece)!;
    });
  });
  project.files.forEach(file => {
    const pieces = file.path.split('/'); pieces.pop();
    let node = root;
    pieces.forEach(piece => node = node.directories.get(piece)!);
    node.files.push(file);
  });
  const renderNode = (node: Tree, prefix = '', depth = 0): string => {
    const directories = [...node.directories.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([name, child]) => {
      const path = prefix ? `${prefix}/${name}` : name;
      return `<div class="tree-entry"><div class="folder-row" style="--depth:${depth}" data-path="${escapeHtml(path)}"><span>⌄</span><span>${escapeHtml(name)}</span><button class="tree-hide" data-hide="${escapeHtml(path)}" title="Скрыть папку">○</button></div>${renderNode(child, path, depth + 1)}</div>`;
    }).join('');
    const files = node.files.sort((a, b) => a.path.localeCompare(b.path)).map(file => {
      const name = file.path.split('/').pop()!;
      return `<div class="tree-entry"><button class="file-row ${file.path === currentPath ? 'active' : ''}" style="--depth:${depth}" data-path="${escapeHtml(file.path)}">${fileIcon(file.path)}<span>${escapeHtml(name)}</span></button><button class="tree-hide file-hide" data-hide="${escapeHtml(file.path)}" title="Скрыть файл">○</button></div>`;
    }).join('');
    return directories + files;
  };
  const hidden = (project.hidden_paths || []).map(path => `<button class="hidden-path" data-unhide="${escapeHtml(path)}">↶ ${escapeHtml(path)}</button>`).join('');
  document.getElementById('sidebarContent')!.innerHTML = `
    <div class="tree-title"><span>⌄</span><b>${escapeHtml(project.name.toUpperCase())}</b></div>
    <div class="file-tree">${renderNode(root)}</div>
    ${hidden ? `<div class="outline"><div class="section-line">СКРЫТО (${project.hidden_paths.length})</div>${hidden}</div>` : ''}
    <div class="outline"><div class="section-line">ПОДСКАЗКА</div><p class="side-help tree-tip">Наведите на файл или папку и нажмите ○, чтобы убрать их из проводника без удаления.</p></div>`;
  document.querySelectorAll<HTMLElement>('.file-row').forEach(element => element.onclick = () => {
    openFile(element.dataset.path!);
    if (compactLayout()) document.body.classList.remove('sidebar-open');
  });
  document.querySelectorAll<HTMLElement>('.tree-hide').forEach(element => element.onclick = async event => {
    event.stopPropagation();
    try { project = await setPathHidden(project.root, element.dataset.hide!, true); renderExplorer(); toast('Путь скрыт. Файл остался на диске.'); }
    catch (error) { toast(String(error), 'warn'); }
  });
  document.querySelectorAll<HTMLElement>('.hidden-path').forEach(element => element.onclick = async () => {
    try { project = await setPathHidden(project.root, element.dataset.unhide!, false); renderExplorer(); }
    catch (error) { toast(String(error), 'warn'); }
  });
}

function renderSearch() {
  document.getElementById('sidebarContent')!.innerHTML = `<div class="search-side"><input id="globalSearch" placeholder="Поиск" autofocus><button class="primary small" id="doSearch">Найти в проекте</button><p class="side-help">Поиск работает по всем .fun-файлам проекта.</p><div id="searchResults"></div></div>`;
  const run = () => {
    const q = (document.getElementById('globalSearch') as HTMLInputElement).value;
    const hits = project.files.flatMap(f => f.content.split('\n').map((line, i) => ({ f, line, i })).filter(x => q && x.line.toLowerCase().includes(q.toLowerCase())));
    document.getElementById('searchResults')!.innerHTML = hits.map(h => `<button class="search-hit" data-path="${h.f.path}" data-line="${h.i + 1}"><b>${h.f.path}:${h.i + 1}</b><span>${h.line.trim()}</span></button>`).join('') || '<p class="side-help">Совпадений пока нет.</p>';
    document.querySelectorAll<HTMLElement>('.search-hit').forEach(x => x.onclick = () => { openFile(x.dataset.path!); editor.revealLineInCenter(Number(x.dataset.line)); editor.setPosition({ lineNumber: Number(x.dataset.line), column: 1 }); if (compactLayout()) document.body.classList.remove('sidebar-open'); });
  };
  document.getElementById('doSearch')!.onclick = run;
  (document.getElementById('globalSearch') as HTMLInputElement).onkeydown = e => { if (e.key === 'Enter') run(); };
}

function renderRunSide() {
  if (!desktopToolsAvailable) {
    document.getElementById('sidebarContent')!.innerHTML = `<div class="run-side"><button class="primary full" id="runSideButton">${icon('play')} ${project.kind.startsWith('minecraft') ? 'Собрать Minecraft-мод' : 'Запустить код'}</button><p class="side-help">Обычный Funo выполняется безопасным встроенным интерпретатором. Minecraft-моды собираются локальным Android JDK/Gradle после установки портативной Java.</p><div class="mobile-capabilities"><span>✓ Редактор и автосохранение</span><span>✓ Запуск обычного Funo</span><span>✓ Локальная сборка Minecraft/JAR</span><span>✓ Встроенный Minecraft Launcher</span></div></div>`;
    document.getElementById('runSideButton')!.onclick = () => { document.body.classList.remove('sidebar-open'); void execute(); };
    return;
  }
  document.getElementById('sidebarContent')!.innerHTML = `<div class="run-side"><button class="primary full" id="runSideButton">${icon('play')} Запустить Funo</button><p class="side-help">Выберите JVM или один из нативных backend-ов. Необходимый компилятор должен быть установлен в системе.</p><div class="side-section-title">КОНФИГУРАЦИЯ</div><label>Backend<select id="backendSelect"><option value="jvm">JVM / Java</option><option value="cpp">C++ 17</option><option value="rust">Rust</option><option value="javascript">JavaScript</option><option value="python">Python</option></select></label><label>Режим<select id="backendMode"><option value="run">Собрать и запустить</option><option value="build">Только собрать</option></select></label><label class="check"><input type="checkbox" checked> Остановиться при ошибке</label><div id="backendTargets" class="backend-targets"></div></div>`;
  const select = document.getElementById('backendSelect') as HTMLSelectElement;
  select.value = compilerBackend;
  select.onchange = () => { compilerBackend = select.value; document.getElementById('backendStatus')!.textContent = compilerBackend.toUpperCase(); };
  document.getElementById('runSideButton')!.onclick = () => void execute();
  document.getElementById('backendTargets')!.innerHTML = ['JVM', 'C++ 17', 'Rust', 'JavaScript', 'Python'].map(target => `<span>○ ${target}</span>`).join('');
}

async function renderPackages() {
  showSurface(`<div class="loading">${icon('refresh')} Загружаю реестр библиотек…</div>`);
  const result = await fetchRegistry().catch(err => ({ source: '', status: 'offline' as const, message: String(err), packages: [] }));
  const cards = result.packages.length ? result.packages.map(packageCard).join('') : `
    <div class="empty-registry">
      <div class="empty-icon">${icon('package')}</div><h2>Репозиторий подключён</h2>
      <p>Пока в нём нет <code>index.json</code>. Добавьте файл по готовому шаблону <b>registry-template/index.json</b>.</p>
      <a href="${result.source}" target="_blank" rel="noreferrer">Открыть ваш GitHub ↗</a>
    </div>`;
  showSurface(`<div class="surface-page packages-page">
    <div class="page-hero"><div><span class="overline">FUNO PACK</span><h1>Библиотеки</h1><p>Проверенные пакеты из вашего GitHub, Java .jar и инструменты для Minecraft.</p></div><div class="hero-actions">${desktopToolsAvailable ? '<button class="primary" id="addOwnPlugin">+ Добавить своё</button>' : ''}<button class="secondary" id="refreshRegistry">${icon('refresh')} Обновить</button></div></div>
    ${desktopToolsAvailable ? '' : '<div class="mobile-notice"><b>Режим каталога</b><span>На Android можно смотреть и искать пакеты. Установка .jar и нативных плагинов выполняется в desktop-версии.</span></div>'}
    <div class="registry-source"><span class="verified-mark">${icon('check')}</span><div><b>Официальный источник</b><code>${result.source}</code></div><span class="registry-state ${result.status}">${result.status === 'ready' ? 'доступен' : result.status === 'empty' ? 'ждёт index.json' : 'нет связи'}</span></div>
    <div class="package-toolbar"><div class="surface-search">${icon('search')}<input id="packageSearch" placeholder="Поиск пакета или Java-библиотеки"></div>${desktopToolsAvailable ? '<button class="secondary" id="addJar">+ Добавить .jar / Maven</button>' : ''}</div>
    <div class="package-grid" id="packageGrid">${cards}</div>
    <section class="trust-info"><h3>Как Funo проверяет пакет</h3><div><span>1</span> HTTPS с GitHub</div><div><span>2</span> SHA-256 совпадает</div><div><span>3</span> Версия записывается в funo.lock</div></section>
  </div>`);
  document.getElementById('refreshRegistry')!.onclick = () => void renderPackages();
  const addOwnPlugin = document.getElementById('addOwnPlugin');
  if (addOwnPlugin) addOwnPlugin.onclick = () => void renderPlugins();
  document.getElementById('packageSearch')!.oninput = e => {
    const q = (e.target as HTMLInputElement).value.toLowerCase();
    document.querySelectorAll<HTMLElement>('.package-card').forEach(c => c.classList.toggle('hidden', !c.dataset.search!.includes(q)));
  };
  document.querySelectorAll<HTMLElement>('.install-package').forEach(btn => btn.onclick = async () => {
    const pkg = result.packages.find(p => p.id === btn.dataset.id)!;
    try { const msg = await installPackage(project.root, pkg, false); toast(msg); btn.textContent = 'Установлено'; }
    catch (err) { toast(String(err), 'warn'); }
  });
  const addJar = document.getElementById('addJar');
  if (addJar) addJar.onclick = () => openModal('Java-библиотека', `<p>Funo поддерживает обычные Java-библиотеки. Укажите Maven-координату:</p><label class="field">Maven ID<input placeholder="com.google.code.gson:gson:2.11.0"></label><label class="field">или локальный файл<input type="file" accept=".jar"></label><div class="modal-actions"><button class="secondary" data-close>Отмена</button><button class="primary" id="confirmJar">Добавить</button></div>`);
}

async function renderPlugins() {
  if (!desktopToolsAvailable) { toast('Plugin SDK доступен в desktop-версии Funo Studio.', 'warn'); void renderPackages(); return; }
  showSurface(`<div class="loading">${icon('refresh')} Загружаю пользовательские плагины…</div>`);
  const plugins = await listPlugins().catch(() => []);
  showSurface(`<div class="surface-page plugins-page"><div class="page-hero"><div><span class="overline">PLUGIN SDK</span><h1>Добавить своё</h1><p>Создайте обычный Git-репозиторий плагина, редактируйте его исходники и запускайте тесты прямо из Studio.</p></div></div>
    <div class="plugin-layout"><section class="wizard"><h2>Новый плагин</h2><label class="field">Название<input id="pluginName" value="Мой плагин"></label><label class="field">Папка для репозитория<input id="pluginParent" value="${escapeHtml(project.root)}"></label><label class="field">Язык<select id="pluginLanguage"><option value="rust">Rust</option><option value="cpp">C++ 17</option><option value="typescript">TypeScript</option><option value="javascript">JavaScript</option><option value="python">Python</option></select></label><label class="field">Тип<select id="pluginKind"><option value="tooling">Инструмент Studio</option><option value="compiler">Backend компилятора</option><option value="minecraft">Minecraft-интеграция</option></select></label><button class="primary big" id="createPlugin">Создать Git-репозиторий</button><p class="side-help">Шаблон содержит SDK ABI, тест, README, manifest и .gitignore.</p></section>
    <section><h2>Установленные</h2><div class="plugin-grid">${plugins.length ? plugins.map(plugin => `<article class="plugin-card"><span class="file-type rust">${escapeHtml(plugin.language.slice(0, 2))}</span><div><b>${escapeHtml(plugin.name)}</b><small>${escapeHtml(plugin.kind)} · ${escapeHtml(plugin.language)}</small><code>${escapeHtml(plugin.root)}</code></div><button class="secondary small check-plugin" data-root="${escapeHtml(plugin.root)}">Тест</button></article>`).join('') : '<div class="empty-registry"><h3>Плагинов пока нет</h3><p>Создайте первый — он останется вашим обычным проектом и Git-репозиторием.</p></div>'}</div></section></div></div>`);
  document.getElementById('createPlugin')!.onclick = async () => {
    const button = document.getElementById('createPlugin') as HTMLButtonElement; button.disabled = true;
    try {
      const plugin = await createPlugin((document.getElementById('pluginParent') as HTMLInputElement).value, (document.getElementById('pluginName') as HTMLInputElement).value, (document.getElementById('pluginLanguage') as HTMLSelectElement).value, (document.getElementById('pluginKind') as HTMLSelectElement).value);
      const check = await checkPlugin(plugin.root);
      if (check.success) await installPlugin(plugin.root);
      project = await reloadProject(project.root);
      renderExplorer(); toast(`${plugin.name}: ${check.summary}`); void renderPlugins();
    } catch (error) { toast(String(error), 'warn'); button.disabled = false; }
  };
  document.querySelectorAll<HTMLElement>('.check-plugin').forEach(button => button.onclick = async () => {
    button.textContent = 'Проверяю…';
    try { const result = await checkPlugin(button.dataset.root!); toast(`${result.summary}${result.output ? `: ${result.output.slice(0, 180)}` : ''}`, result.success ? 'ok' : 'warn'); }
    catch (error) { toast(String(error), 'warn'); }
    button.textContent = 'Тест';
  });
}

function packageCard(p: RegistryPackage) {
  return `<article class="package-card" data-search="${`${p.name} ${p.id} ${p.description}`.toLowerCase()}"><div class="package-icon">${p.kind === 'minecraft' ? icon('cube') : p.kind === 'java' ? icon('java') : icon('package')}</div><div class="package-main"><h3>${p.name}${p.verified ? `<span class="verified" title="SHA-256 указан">${icon('check')}</span>` : ''}</h3><code>${p.id}@${p.version}</code><p>${p.description}</p><footer><span>${p.kind}</span><button class="${desktopToolsAvailable ? 'primary' : 'secondary'} small install-package" data-id="${p.id}" ${desktopToolsAvailable ? '' : 'disabled'}>${desktopToolsAvailable ? 'Установить' : 'Desktop'}</button></footer></div></article>`;
}

function formatBytes(bytes: number) {
  const gib = bytes / 1024 ** 3;
  return `${gib >= 10 ? gib.toFixed(0) : gib.toFixed(1)} ГиБ`;
}

function minecraftRequirements() {
  const manifest = project.files.find(file => file.path === 'funo.toml')?.content || '';
  return {
    version: manifestValue(manifest, 'minecraft', 'version') || '1.21.1',
    loader: manifestValue(manifest, 'minecraft', 'loader') || project.kind.replace('minecraft-', '') || 'fabric'
  };
}

async function renderMinecraftToolchains(
  projectRoot = project.root,
  minecraftVersion = minecraftRequirements().version,
  loader = minecraftRequirements().loader,
  launcherInstanceId?: string,
  checkUpdates = false,
  returnView: 'minecraft' | 'settings' = 'minecraft'
) {
  if (!desktopToolsAvailable && !runtimePlatform.android) { toast('Установщик доступен в приложении Funo Studio.', 'warn'); renderMinecraft(); return; }
  const goBack = () => launcherInstanceId
    ? void renderLauncher(launcherInstanceId)
    : returnView === 'settings' ? void renderSettings() : renderMinecraft();
  showSurface(`<div class="loading">${icon('refresh')} Проверяю JDK, Gradle и свободное место…</div>`);
  let status: MinecraftToolchainStatus;
  try {
    status = await minecraftToolchainStatus(projectRoot, minecraftVersion, loader, checkUpdates);
  } catch (error) {
    showSurface(`<div class="surface-page toolchains-page"><div class="page-hero"><div><span class="overline">MINECRAFT TOOLCHAIN</span><h1>Не удалось проверить инструменты</h1><p class="error">${escapeHtml(String(error))}</p></div><button class="secondary" id="toolchainBack">← Назад</button></div></div>`);
    document.getElementById('toolchainBack')!.onclick = goBack;
    return;
  }
  const alternatives = status.volumes.filter(volume => !volume.current && volume.eligible);
  const currentBlocked = status.volumes.some(volume => volume.current && !volume.eligible);
  const toolCard = (tool: MinecraftToolchainStatus['jdk'], kind: 'jdk' | 'gradle') => `<article class="tool-status ${tool.compatible ? 'ready' : 'missing'}">
    <div class="tool-status-icon">${tool.compatible ? icon('check') : icon('warning')}</div>
    <div><span>${kind === 'jdk' ? 'JAVA DEVELOPMENT KIT' : 'GRADLE'}</span><h2>${kind === 'jdk' ? `JDK ${status.required_java}` : `Gradle ${escapeHtml(status.recommended_gradle)}`}</h2><p>${escapeHtml(tool.detail)}</p>${tool.path ? `<code>${escapeHtml(tool.path)}</code>` : ''}</div>
    <div class="tool-version"><b>${tool.version ? escapeHtml(tool.version) : 'не найден'}</b><small>${tool.managed ? 'управляется Funo' : tool.path.endsWith('gradle-wrapper.properties') ? 'wrapper проекта' : tool.found ? 'системный' : 'требуется установка'}</small>${tool.update_available ? `<em>обновление ${escapeHtml(tool.latest_version)}</em>` : ''}</div>
  </article>`;
  const volumeCards = status.volumes.map((volume, index) => `<label class="volume-card ${volume.eligible ? '' : 'blocked'} ${volume.current ? 'current' : ''}">
    <input type="radio" name="toolVolume" value="${escapeHtml(volume.install_root)}" ${volume.eligible && (volume.current || !status.volumes.some(item => item.current && item.eligible)) && (volume.current || index === status.volumes.findIndex(item => item.eligible)) ? 'checked' : ''} ${volume.eligible ? '' : 'disabled'}>
    <span><b>${escapeHtml(volume.id)}${volume.current ? ' · текущий' : ''}</b><small>После установки останется ${formatBytes(volume.available_after_bytes)}</small><code>${escapeHtml(volume.install_root)}</code></span>
    <strong>${formatBytes(volume.free_bytes)}<small>свободно</small></strong>
  </label>`).join('');
  const needsInstall = !status.ready || status.has_updates;
  showSurface(`<div class="surface-page toolchains-page">
    <div class="page-hero"><div><span class="overline">MINECRAFT TOOLCHAIN</span><h1>JDK и Gradle</h1><p>${escapeHtml(loader)} · Minecraft ${escapeHtml(minecraftVersion)} · Java ${status.required_java}. Studio проверяет совместимость перед сборкой и запуском.</p></div><div class="hero-actions"><button class="secondary" id="toolchainBack">← Назад</button><button class="secondary" id="checkToolUpdates">${icon('refresh')} ${checkUpdates ? 'Проверено' : 'Проверить обновления'}</button></div></div>
    <div class="tool-status-list">${toolCard(status.jdk, 'jdk')}${toolCard(status.gradle, 'gradle')}</div>
    <section class="reserve-policy"><div>${icon('warning')}</div><p><b>После установки обязательно останется не меньше 30 ГиБ.</b><br>Нужно свободно ${formatBytes(status.reserve_bytes + status.estimated_install_bytes)}: резерв 30 ГиБ плюс архивы и файлы JDK/Gradle. Установка на неподходящий диск заблокирована и повторно проверяется backend-ом.</p></section>
    ${currentBlocked && alternatives.length ? `<div class="alternative-notice"><b>На текущем диске недостаточно места.</b><span>Выберите другой доступный диск: ${alternatives.map(volume => escapeHtml(volume.id)).join(', ')}.</span></div>` : ''}
    <section class="volume-picker"><header><div><h2>Куда установить управляемые инструменты</h2><p>Проекты не перемещаются. На выбранном диске появится отдельная папка Funo Studio.</p></div><span>${status.volumes.filter(volume => volume.eligible).length} доступно</span></header><div>${volumeCards || '<p class="error">Не удалось найти доступные диски.</p>'}</div></section>
    <div class="toolchain-actions"><div><b class="${status.ready ? 'success' : 'error'}">${status.ready ? 'Инструменты готовы' : 'Нужна настройка'}</b><span>${escapeHtml(status.message)}</span></div><button class="primary big" id="installToolchain" ${needsInstall && status.volumes.some(volume => volume.eligible) ? '' : 'disabled'}>${status.ready ? 'Обновить JDK и Gradle' : 'Установить JDK и Gradle'}</button></div>
  </div>`);
  document.getElementById('toolchainBack')!.onclick = goBack;
  document.getElementById('checkToolUpdates')!.onclick = () => void renderMinecraftToolchains(projectRoot, minecraftVersion, loader, launcherInstanceId, true, returnView);
  const installButton = document.getElementById('installToolchain') as HTMLButtonElement;
  installButton.onclick = async () => {
    const destination = document.querySelector<HTMLInputElement>('input[name="toolVolume"]:checked')?.value;
    if (!destination) { toast('Выберите диск, на котором после установки останется 30 ГиБ.', 'warn'); return; }
    installButton.disabled = true;
    installButton.textContent = status.ready ? 'Обновляю и проверяю SHA-256…' : 'Скачиваю и проверяю SHA-256…';
    try {
      const installed = await installMinecraftToolchain(projectRoot, minecraftVersion, loader, destination);
      toast(installed.message);
      await renderMinecraftToolchains(projectRoot, minecraftVersion, loader, launcherInstanceId, false, returnView);
    } catch (error) {
      toast(String(error), 'warn');
      installButton.disabled = false;
      installButton.textContent = 'Повторить установку';
    }
  };
}

function renderMinecraft() {
  showSurface(`<div class="surface-page minecraft-page"><div class="page-hero"><div><span class="overline">MINECRAFT + FUNO</span><h1>Новый мод без сложного Java-кода</h1><p>${desktopToolsAvailable ? 'Создавайте моды, запускайте изолированные сборки и устанавливайте совместимые моды с Modrinth.' : 'Редактируйте и собирайте моды локально, затем запускайте Minecraft Java во встроенном Launcher.'}</p></div><div class="hero-actions"><button class="secondary" id="openToolchains">${runtimePlatform.android ? 'Установить портативную Java' : 'JDK и Gradle'}</button><button class="primary" id="openLauncher">${icon('play')} ${runtimePlatform.android ? 'Запустить Minecraft' : 'Лаунчер и сборки'}</button><div class="voxel">F</div></div></div>
    ${desktopToolsAvailable ? '' : '<div class="mobile-notice"><b>Всё внутри Funo Studio APK</b><span>Портативный Android JDK и Gradle устанавливаются в приватное хранилище. Сборка мода выполняется локально, а встроенный Launcher использует отдельные каталоги Minecraft.</span></div>'}
    <div class="wizard-grid"><section class="wizard"><h2>Создать проект</h2><label class="field">Название мода<input id="modName" value="Мой первый мод"></label><label class="field">ID мода<input id="modId" value="my_first_mod" pattern="[a-z0-9_]+"></label><label class="field">Загрузчик<div class="loader-options"><button class="loader active" data-loader="fabric"><b>Fabric</b><span>Лёгкий и быстрый</span></button><button class="loader" data-loader="forge"><b>Forge</b><span>Большая экосистема</span></button><button class="loader" data-loader="neoforge"><b>NeoForge</b><span>Современный Forge</span></button></div></label><label class="field">Версия Minecraft<select id="minecraftVersion" disabled><option>Загрузка версий…</option></select><small id="minecraftVersionHint">Получаем официальный каталог загрузчика</small></label><button class="primary big" id="createMod" disabled>${icon('cube')} Создать Minecraft-мод</button></section>
    <section class="code-preview"><span>main.fun · <b id="minecraftCode">Minecraft</b> · Funo API</span><pre><i>use</i> minecraft.<b id="loaderCode">fabric</b>

<i>mod</i> <s>"my_first_mod"</s> {
    <i>on</i> server_start {
        <em>broadcast</em>(<s>"Сервер готов!"</s>)
        <em>run_command</em>(<s>"time set day"</s>)
    }
    <i>on</i> player_join(player) {
        <em>tell</em>(<s>f"Добро пожаловать, {player}!"</s>)
    }
    <i>on</i> block_break(player, block) {
        <em>log</em>(<s>f"{player} сломал {block}"</s>)
    }
}</pre><div class="what-created"><b>Funo создаст:</b><span>✓ локальный main.fun</span><span>✓ события Fabric / Forge / NeoForge</span><span>✓ manifest с версией Java</span><span>✓ готовый JAR мода локально</span></div></section></div>
    <div class="learn-strip"><div>${icon('spark')}</div><p><b>Свои команды Funo без сложного Java-кода.</b><br>Доступны <code>log</code>, <code>broadcast</code>, <code>tell</code>, <code>give</code>, <code>actionbar</code> и <code>run_command</code>.</p></div>
  </div>`);
  let loader = 'fabric';
  let versions: MinecraftVersion[] = [];
  let versionRequest = 0;
  const versionSelect = document.getElementById('minecraftVersion') as HTMLSelectElement;
  const versionHint = document.getElementById('minecraftVersionHint')!;
  const createButton = document.getElementById('createMod') as HTMLButtonElement;
  const openLauncher = document.getElementById('openLauncher');
  if (openLauncher) openLauncher.onclick = () => {
    if (desktopToolsAvailable) { void renderLauncher(); return; }
    void (async () => {
      const requirements = minecraftRequirements();
      try {
        const tools = await minecraftToolchainStatus(project.root, requirements.version, requirements.loader);
        if (!tools.ready) {
          toast('Сначала установите портативную Java. Открываю встроенный установщик.', 'warn');
          await renderMinecraftToolchains(project.root, requirements.version, requirements.loader);
          return;
        }
        toast(await openAndroidLauncher());
      } catch (error) { toast(String(error), 'warn'); }
    })();
  };
  const openToolchains = document.getElementById('openToolchains');
  if (openToolchains) openToolchains.onclick = () => {
    const selectedVersion = versions.find(version => version.id === versionSelect.value)?.id || minecraftRequirements().version;
    void renderMinecraftToolchains(project.root, selectedVersion, loader);
  };

  const updateVersionHint = () => {
    const selected = versions.find(version => version.id === versionSelect.value);
    versionHint.textContent = selected ? `${loader} · Minecraft ${selected.label} · требуется Java ${selected.java}` : 'Выберите совместимую версию';
    document.getElementById('minecraftCode')!.textContent = selected ? `Minecraft ${selected.label}` : 'Minecraft';
  };

  const loadVersions = async () => {
    const request = ++versionRequest;
    versionSelect.disabled = true;
    createButton.disabled = true;
    versionSelect.innerHTML = '<option>Загрузка версий…</option>';
    versionHint.textContent = `Получаем официальный каталог ${loader}…`;
    try {
      const result = await fetchMinecraftVersions(loader);
      if (request !== versionRequest) return;
      versions = result;
      if (!versions.length) throw new Error(`Для ${loader} пока нет доступных версий`);
      versionSelect.innerHTML = versions.map(version => `<option value="${escapeHtml(version.id)}">${escapeHtml(version.label)} · Java ${version.java}${version.stable ? '' : ' · preview'}</option>`).join('');
      versionSelect.disabled = false;
      createButton.disabled = false;
      updateVersionHint();
    } catch (error) {
      if (request !== versionRequest) return;
      versions = [];
      versionSelect.innerHTML = '<option>Версии недоступны</option>';
      versionHint.textContent = String(error);
      toast(String(error), 'warn');
    }
  };

  document.querySelectorAll<HTMLElement>('.loader').forEach(element => element.onclick = () => {
    loader = element.dataset.loader!;
    document.querySelectorAll('.loader').forEach(option => option.classList.remove('active'));
    element.classList.add('active');
    document.getElementById('loaderCode')!.textContent = loader;
    void loadVersions();
  });
  versionSelect.onchange = updateVersionHint;
  const idInput = document.getElementById('modId') as HTMLInputElement;
  idInput.oninput = () => { document.querySelector('.code-preview s')!.textContent = `"${idInput.value}"`; };
  createButton.onclick = async () => {
    const name = (document.getElementById('modName') as HTMLInputElement).value.trim();
    const modId = idInput.value.trim();
    const minecraftVersion = versionSelect.value;
    if (!name) { toast('Укажите название мода.', 'warn'); return; }
    if (!/^[a-z][a-z0-9_]{2,63}$/.test(modId)) { toast('ID: маленькие латинские буквы, цифры и _.', 'warn'); return; }
    if (!versions.some(version => version.id === minecraftVersion)) { toast('Выберите версию Minecraft.', 'warn'); return; }
    createButton.disabled = true;
    createButton.innerHTML = `${icon('refresh')} Подбираем зависимости…`;
    try {
      project = await createMinecraftProject(name, modId, loader, minecraftVersion);
      models.forEach(model => model.dispose()); models.clear();
      currentPath = project.files[0].path; updateProjectUI(); openFile(currentPath);
      toast(`${loader} · Minecraft ${minecraftVersion}: проект создан!`);
    } catch (err) {
      toast(String(err), 'warn');
      createButton.disabled = false;
      createButton.innerHTML = `${icon('cube')} Создать Minecraft-мод`;
    }
  };
  void loadVersions();
}

async function renderLauncher(selectedId?: string) {
  if (!desktopToolsAvailable) { toast('Minecraft Launcher доступен в desktop-версии.', 'warn'); renderMinecraft(); return; }
  [instances, account] = await Promise.all([listInstances(), currentMicrosoftAccount().catch(() => null)]);
  const selected = instances.find(instance => instance.id === selectedId) || instances[0];
  showSurface(`<div class="surface-page launcher-page"><div class="page-hero"><div><span class="overline">FUNO LAUNCHER</span><h1>Независимые сборки</h1><p>У каждой сборки отдельные <code>mods</code>, <code>config</code>, аргументы JVM и каталог игры.</p></div><div class="hero-actions"><button class="secondary" id="backToModWizard">← Мастер модов</button>${selected ? '<button class="secondary" id="launcherToolchains">JDK и Gradle</button>' : ''}<button class="${account ? 'secondary' : 'primary'}" id="accountButton">${account ? `● ${escapeHtml(account.username)}` : 'Войти через Microsoft'}</button></div></div>
    <div class="launcher-layout"><aside class="instance-sidebar"><h2>Сборки</h2>${instances.map(instance => `<button class="instance-card ${instance.id === selected?.id ? 'active' : ''}" data-instance="${escapeHtml(instance.id)}"><b>${escapeHtml(instance.name)}</b><span>${escapeHtml(instance.loader)} · ${escapeHtml(instance.minecraft_version)}</span><small>${instance.mods.length} модов</small></button>`).join('') || '<p class="side-help">Создайте первую сборку для текущего Minecraft-проекта.</p>'}<button class="secondary full" id="newInstance">+ Новая сборка</button></aside>
    <main class="instance-main">${selected ? `<div class="instance-heading"><div><h2>${escapeHtml(selected.name)}</h2><code>${escapeHtml(selected.game_dir)}</code></div><button class="primary big" id="launchMinecraft">${icon('play')} Запустить Minecraft</button></div>
      <div class="instance-tabs"><button class="active" data-instance-tab="config">Запуск</button><button data-instance-tab="mods">Моды (${selected.mods.length})</button></div>
      <section id="instanceConfig" class="instance-content"><div class="settings-list compact"><section><label><span><b>Аргументы JVM</b><small>Память, сборщик мусора и системные свойства</small></span><input id="instanceJvm" value="${escapeHtml(selected.jvm_args)}" placeholder="-Xmx4G"></label><label><span><b>Аргументы игры</b><small>Например, --width 1280 --height 720</small></span><input id="instanceGame" value="${escapeHtml(selected.game_args)}"></label><label><span><b>Задача Gradle</b><small>Можно указать задачу custom loader-а</small></span><input id="instanceTask" value="${escapeHtml(selected.launch_task)}"></label></section></div><div class="modal-actions"><button class="danger" id="deleteInstance">Удалить сборку</button><button class="primary" id="saveInstance">Сохранить конфигурацию</button></div></section>
      <section id="instanceMods" class="instance-content hidden"><div class="package-toolbar"><div class="surface-search">${icon('search')}<input id="modrinthSearch" placeholder="Поиск модов на Modrinth"></div><button class="primary" id="findMods">Найти</button></div><p class="side-help">Результаты автоматически ограничены версией ${escapeHtml(selected.minecraft_version)} и загрузчиком ${escapeHtml(selected.loader)}. Повторная загрузка не создаёт копию.</p><div id="modrinthResults" class="modrinth-grid"></div><h3>Установлено</h3><div class="installed-mods">${selected.mods.map(mod => `<article><div><b>${escapeHtml(mod.name)}</b><small>${escapeHtml(mod.file_name)}</small></div><button class="secondary small remove-mod" data-project="${escapeHtml(mod.project_id)}">Удалить</button></article>`).join('') || '<p class="side-help">В этой сборке пока нет модов.</p>'}</div></section>` : `<div class="empty-registry"><div class="empty-icon">${icon('cube')}</div><h2>Создайте независимую сборку</h2><p>Она будет привязана к текущему Minecraft-проекту, но получит собственные моды и настройки.</p></div>`}</main></div></div>`);
  document.getElementById('backToModWizard')!.onclick = renderMinecraft;
  document.getElementById('accountButton')!.onclick = () => void handleMicrosoftAccount();
  if (selected) document.getElementById('launcherToolchains')!.onclick = () => void renderMinecraftToolchains(selected.project_root, selected.minecraft_version, selected.loader, selected.id);
  document.getElementById('newInstance')!.onclick = () => {
    const kindLoader = project.kind.replace('minecraft-', '') || 'fabric';
    const manifest = project.files.find(file => file.path === 'funo.toml')?.content || '';
    const version = manifestValue(manifest, 'minecraft', 'version') || '1.21.1';
    openModal('Новая сборка Minecraft', `<label class="field">Название<input id="newInstanceName" value="${escapeHtml(project.name)} — тест"></label><label class="field">Minecraft<input id="newInstanceVersion" value="${escapeHtml(version)}"></label><label class="field">Загрузчик<input id="newInstanceLoader" value="${escapeHtml(kindLoader)}"><small>Можно указать идентификатор своего загрузчика</small></label><p>Проект: <code>${escapeHtml(project.root)}</code></p><div class="modal-actions"><button class="secondary" data-close>Отмена</button><button class="primary" id="confirmInstance">Создать</button></div>`);
    document.getElementById('confirmInstance')!.onclick = async () => {
      try {
        const instance = await createInstance((document.getElementById('newInstanceName') as HTMLInputElement).value, project.root, (document.getElementById('newInstanceVersion') as HTMLInputElement).value, (document.getElementById('newInstanceLoader') as HTMLInputElement).value);
        document.getElementById('modalLayer')!.classList.add('hidden'); await renderLauncher(instance.id);
      } catch (error) { toast(String(error), 'warn'); }
    };
  };
  document.querySelectorAll<HTMLElement>('.instance-card').forEach(button => button.onclick = () => void renderLauncher(button.dataset.instance));
  if (!selected) return;
  document.querySelectorAll<HTMLElement>('[data-instance-tab]').forEach(button => button.onclick = () => {
    document.querySelectorAll('[data-instance-tab]').forEach(tab => tab.classList.toggle('active', tab === button));
    document.getElementById('instanceConfig')!.classList.toggle('hidden', button.dataset.instanceTab !== 'config');
    document.getElementById('instanceMods')!.classList.toggle('hidden', button.dataset.instanceTab !== 'mods');
  });
  document.getElementById('saveInstance')!.onclick = async () => {
    selected.jvm_args = (document.getElementById('instanceJvm') as HTMLInputElement).value;
    selected.game_args = (document.getElementById('instanceGame') as HTMLInputElement).value;
    selected.launch_task = (document.getElementById('instanceTask') as HTMLInputElement).value;
    try { await updateInstance(selected); toast('Конфигурация сборки сохранена.'); }
    catch (error) { toast(String(error), 'warn'); }
  };
  document.getElementById('deleteInstance')!.onclick = async () => {
    if (!confirm(`Удалить сборку «${selected.name}» и её отдельную папку игры?`)) return;
    try { await deleteInstance(selected.id); await renderLauncher(); }
    catch (error) { toast(String(error), 'warn'); }
  };
  document.getElementById('launchMinecraft')!.onclick = async () => {
    try {
      const tools = await minecraftToolchainStatus(selected.project_root, selected.minecraft_version, selected.loader);
      if (!tools.ready) {
        toast('Перед запуском настройте JDK и Gradle. Studio уже подобрала совместимые версии.', 'warn');
        await renderMinecraftToolchains(selected.project_root, selected.minecraft_version, selected.loader, selected.id);
        return;
      }
    } catch (error) { toast(`Не удалось проверить JDK и Gradle: ${String(error)}`, 'warn'); return; }
    selectPanel('terminal'); document.getElementById('panelBody')!.innerHTML = '<span class="muted">Запускаю изолированный Minecraft runClient…</span>';
    try { const output = await launchInstance(selected.id); document.getElementById('panelBody')!.innerHTML = `<span class="success">Minecraft завершил работу</span>\n${escapeHtml(output)}`; }
    catch (error) { document.getElementById('panelBody')!.innerHTML = `<span class="error">Запуск Minecraft остановлен</span>\n${escapeHtml(String(error))}`; }
  };
  const search = async () => {
    const query = (document.getElementById('modrinthSearch') as HTMLInputElement).value.trim(); if (!query) return;
    const target = document.getElementById('modrinthResults')!; target.innerHTML = '<div class="loading">Ищу совместимые версии…</div>';
    try {
      const results = await searchModrinth(query, selected.loader, selected.minecraft_version);
      target.innerHTML = results.map(mod => `<article class="modrinth-card">${mod.icon_url ? `<img src="${escapeHtml(mod.icon_url)}" alt="">` : `<div class="package-icon">${icon('cube')}</div>`}<div><b>${escapeHtml(mod.title)}</b><span>${escapeHtml(mod.author)} · ${mod.downloads.toLocaleString('ru-RU')} загрузок</span><p>${escapeHtml(mod.description)}</p></div><button class="primary small install-mod" data-project="${escapeHtml(mod.project_id)}">Установить</button></article>`).join('') || '<p class="side-help">Совместимых проектов не найдено.</p>';
      document.querySelectorAll<HTMLElement>('.install-mod').forEach(button => button.onclick = async () => {
        button.textContent = 'Загрузка…'; button.setAttribute('disabled', '');
        try { await installModrinth(selected.id, button.dataset.project!); toast('Мод установлен без дубликатов.'); await renderLauncher(selected.id); }
        catch (error) { toast(String(error), 'warn'); button.textContent = 'Ошибка'; }
      });
    } catch (error) { target.innerHTML = `<p class="error">${escapeHtml(String(error))}</p>`; }
  };
  document.getElementById('findMods')!.onclick = () => void search();
  (document.getElementById('modrinthSearch') as HTMLInputElement).onkeydown = event => { if (event.key === 'Enter') void search(); };
  document.querySelectorAll<HTMLElement>('.remove-mod').forEach(button => button.onclick = async () => { try { await removeInstanceMod(selected.id, button.dataset.project!); await renderLauncher(selected.id); } catch (error) { toast(String(error), 'warn'); } });
}

async function handleMicrosoftAccount() {
  if (!desktopToolsAvailable) { toast('Авторизация лаунчера доступна в desktop-версии.', 'warn'); return; }
  if (account) {
    openModal('Аккаунт Minecraft', `<p>Вы вошли как <b>${escapeHtml(account.username)}</b>.</p><div class="modal-actions"><button class="secondary" data-close>Закрыть</button><button class="danger" id="logoutMicrosoft">Выйти</button></div>`);
    document.getElementById('logoutMicrosoft')!.onclick = async () => { await logoutMicrosoft(); account = null; document.getElementById('modalLayer')!.classList.add('hidden'); await renderLauncher(); };
    return;
  }
  if (!settings.microsoft_client_id) { toast('Сначала укажите Microsoft Client ID в настройках Studio.', 'warn'); selectView('settings'); return; }
  try {
    const challenge = await beginMicrosoftAuth();
    openModal('Вход через Microsoft', `<p>${escapeHtml(challenge.message)}</p><div class="auth-code">${escapeHtml(challenge.user_code)}</div><a class="primary auth-link" href="${escapeHtml(challenge.verification_uri)}" target="_blank" rel="noreferrer">Открыть страницу Microsoft ↗</a><p class="side-help">После подтверждения вернитесь сюда. Funo проверит лицензию Minecraft.</p><div class="modal-actions"><button class="secondary" data-close>Отмена</button><button class="primary" id="completeMicrosoft">Я подтвердил вход</button></div>`);
    document.getElementById('completeMicrosoft')!.onclick = async () => {
      const button = document.getElementById('completeMicrosoft') as HTMLButtonElement; button.disabled = true; button.textContent = 'Проверяю…';
      try { account = await completeMicrosoftAuth(challenge.device_code); document.getElementById('modalLayer')!.classList.add('hidden'); toast(`Вход выполнен: ${account.username}`); await renderLauncher(); }
      catch (error) { toast(String(error), 'warn'); button.disabled = false; button.textContent = 'Повторить'; }
    };
  } catch (error) { toast(String(error), 'warn'); }
}

const wiki = [
  ['Старт', 'Самый короткий код', `<h1>Самый короткий код</h1><p>Funo сам выводит типы и возвращаемые значения.</p><pre>fun hello(name: text) -> text = "Привет, " + name\n\nfun main() = println(hello("Мир"))</pre><div class="doc-note">Точки с запятой и <code>return(200)</code> необязательны.</div>`],
  ['Основы', 'Типы и переменные', `<h1>Типы и переменные</h1><p>Для обычных программ доступны числа, текст, логика, символы, массивы и коллекции.</p><pre>int score = 10\nlong worldSeed = 123456789L\nfloat speed = 1.5f\ndouble health = 19.75\ntext player = "Alex"\nbool online = true\nchar rank = 'A'\nint[] ids = [1, 2, 3]\nlist&lt;text&gt; names = ["Alex", "Steve"]\nmap&lt;text, int&gt; scores = map()\nscores.put("Alex", 42)</pre><p><code>let</code> создаёт неизменяемое значение, <code>var</code> — изменяемое, а тип можно написать через двоеточие: <code>let age: int = 18</code>.</p>`],
  ['Основы', 'Функции и циклы', `<h1>Функции и циклы</h1><pre>fun double(n: int) -> int = n * 2\n\nfun main() {\n    for i in 0..10 {\n        println(double(i))\n    }\n\n    int left = 3\n    while left &gt; 0 {\n        println(left)\n        left = left - 1\n    }\n}</pre><p>Также доступны <code>repeat</code>, <code>break</code>, <code>continue</code>, <code>and</code>, <code>or</code> и <code>not</code>.</p>`],
  ['Основы', 'F-строки', `<h1>F-строки</h1><p>Префикс <code>f</code> вставляет результат любого выражения в текст. Один и тот же синтаксис работает в Java/JVM, Minecraft, C++ 17, Rust, JavaScript и Python.</p><pre>text name = "Alex"\nint score = 42\nprintln(f"Игрок {name}: {score}")\nprintln(f"Фигурные скобки: {{готово}}")</pre><div class="doc-note"><code>{expression}</code> вычисляет выражение, а <code>{{</code> и <code>}}</code> печатают обычные фигурные скобки. В Minecraft <code>{player}</code> показывает имя игрока.</div>`],
  ['CLI', 'Компилятор в терминале', `<h1>Funo в терминале</h1><p>CLI собирается вместе с проектом и сам устанавливается в пользовательский PATH.</p><pre>funo setup\nfuno check main.fun\nfuno run main.fun\nfuno build main.fun -o app.jar</pre><div class="doc-note">Для сборки нужен JDK 17 или 21. После <code>funo setup</code> откройте новый терминал.</div>`],
  ['Backend', 'C++, Rust, JS и Python', `<h1>Нативные и script backend-ы</h1><p>Откройте «Запуск», выберите цель и запустите тот же исходник Funo. Studio создаёт читаемый C++ 17, Rust, JavaScript или Python в <code>.funo/native</code>.</p><pre>fun double(n: int) -> int = n * 2\n\nfun main() {\n    for i in 1..=5 {\n        println(double(i))\n    }\n}</pre><div class="doc-note">Для C++ нужен <code>c++</code>, для Rust — <code>rustc</code>, для JavaScript — Node.js, для Python — Python 3.</div>`],
  ['Java', 'Java-библиотеки', `<h1>Java-библиотеки</h1><p>Установленные Java-пакеты автоматически попадают в classpath.</p><pre>use java "com.google.gson.Gson"\n\nfun main() {\n    var gson = new Gson()\n    println(gson.toJson("Привет"))\n}</pre>`],
  ['Minecraft', 'События и команды', `<h1>Minecraft API Funo</h1><p>Мост Fabric/Forge/NeoForge подключает доступные внутримировые действия на всех поддерживаемых Studio версиях. В каждом событии <code>player</code> — конкретный игрок-исполнитель.</p><pre>use minecraft.fabric\n\nmod "hello_mod" {\n    on player_join(player) {\n        tell(f"Добро пожаловать, {player}!")\n    }\n    on block_break(player, block) {\n        broadcast(f"{player} сломал {block}")\n        give("minecraft:diamond", 1)\n    }\n    on player_leave(player) {\n        log(f"{player} вышел из мира")\n    }\n}</pre><h2>Именованные события</h2><p><code>player_join</code>, <code>player_leave</code>, <code>player_tick</code>, <code>block_break</code>, <code>block_place</code>, <code>block_interact</code>, <code>item_use</code>, <code>item_pickup</code>, <code>item_drop</code>, <code>item_craft</code>, <code>item_smelt</code>, <code>entity_interact</code>, <code>entity_attack</code>, <code>entity_kill</code>, <code>player_damage</code>, <code>player_death</code>, <code>player_respawn</code>, <code>dimension_change</code>, <code>chat</code>, <code>command</code>, <code>container_open</code>, <code>container_close</code>, <code>player_sleep</code>, <code>player_wake</code>, <code>advancement</code> и <code>player_jump</code>.</p><p>Универсальный обработчик <code>on player_event(player, event, detail)</code> получает остальные доступные loader-события игрока и остаётся совместимым между версиями. Второй параметр именованного события содержит контекст: например <code>block</code>, <code>item</code>, <code>entity</code>, <code>amount</code> или <code>message</code>.</p><div class="doc-note"><code>tell</code>, <code>give</code>, <code>damage</code> и <code>tp</code> действуют только на <code>player</code> текущего события. Для <code>damage</code> нужен Minecraft 1.19.4+. Одиночный мир — встроенный локальный сервер, поэтому серверные и player-события работают и там. При сборке Studio автоматически обновляет runtime и loader-мост уже созданного проекта.</div>`],
  ['Minecraft', 'Предметы и рецепты', `<h1>Предметы и рецепты</h1><p><code>define_item</code> создаёт именованный Funo-предмет на основе существующего item ID, а craft-команды генерируют version-aware JSON-рецепты.</p><pre>on server_start {\n    define_item("ruby", "minecraft:emerald", "Рубин")\n    craft_shapeless("ruby_block", "minecraft:emerald_block", 1,\n        "minecraft:emerald", "minecraft:emerald")\n}\n\non player_join(player) {\n    give_custom("ruby", 3)\n}</pre><div class="doc-note">Для 1.20.5+ Funo использует <code>result.id</code>, а для 1.21+ — папку <code>recipe</code>.</div>`],
  ['Minecraft', 'Мобы и AI', `<h1>Мобы и AI</h1><p>Создавайте сущностей, включайте или отключайте их AI и меняйте атрибуты ближайшего моба выбранного типа.</p><pre>on server_start {\n    spawn_mob("minecraft:zombie", 0, 80, 0, "Страж Funo")\n    set_mob_ai("minecraft:zombie", true)\n    mob_attribute("minecraft:zombie", "minecraft:generic.max_health", 40.0)\n}</pre>`],
  ['Minecraft', 'Сборки и Modrinth', `<h1>Независимые сборки</h1><p>В «Minecraft → Лаунчер и сборки» каждая сборка получает отдельные <code>mods</code>, <code>config</code> и игровой каталог. Аргументы JVM, игры и задача Gradle редактируются отдельно.</p><p>Поиск Modrinth учитывает загрузчик и версию выбранной сборки. Повторная установка обновляет существующий проект мода вместо создания копии.</p><div class="doc-note">Для custom loader-а укажите его ID при создании сборки и подходящую Gradle-задачу запуска.</div>`],
  ['Пакеты', 'Официальный реестр', `<h1>Официальный реестр</h1><p>Пакеты загружаются только по HTTPS, сверяются по SHA-256 и записываются в <code>funo.lock</code>.</p><pre>funo pkg list\nfuno pkg search minecraft\nfuno pkg install funo.hello</pre><p>Источник: <code>github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL</code>. Для реестра нужен <code>index.json</code>; готовый пример находится в <code>registry-template</code>.</p>`],
  ['Плагины', 'Свой репозиторий', `<h1>Свой плагин</h1><p>Нажмите «Библиотеки → Добавить своё», выберите Rust, C++ 17, TypeScript, JavaScript или Python. Studio создаст Git-репозиторий с <code>funo.plugin.json</code>, ABI-примером, тестом и README.</p><ol><li>Измените файлы плагина в проводнике.</li><li>Запустите тест — Cargo, CMake, npm или Python.</li><li>Успешно проверенный плагин устанавливается в локальный каталог Studio.</li></ol><div class="doc-note">Секреты GitHub не нужны: репозиторий обычный, и вы сами выбираете, где его публиковать.</div>`]
];

function renderWiki() {
  showSurface(`<div class="surface-page wiki-page"><aside class="doc-nav"><div class="surface-search">${icon('search')}<input id="docSearch" placeholder="Поиск в вики"></div>${wiki.map((x, i) => `<button class="doc-link ${i === 0 ? 'active' : ''}" data-doc="${i}"><small>${x[0]}</small>${x[1]}</button>`).join('')}</aside><article class="doc-content" id="docContent">${wiki[0][2]}</article></div>`);
  document.querySelectorAll<HTMLElement>('.doc-link').forEach(x => x.onclick = () => { document.querySelectorAll('.doc-link').forEach(y => y.classList.remove('active')); x.classList.add('active'); document.getElementById('docContent')!.innerHTML = wiki[Number(x.dataset.doc)][2]; });
  (document.getElementById('docSearch') as HTMLInputElement).oninput = e => { const q = (e.target as HTMLInputElement).value.toLowerCase(); document.querySelectorAll<HTMLElement>('.doc-link').forEach((x, i) => x.classList.toggle('hidden', !wiki[i].join(' ').toLowerCase().includes(q))); };
}

const learningTasks = [
  { title: 'Поздоровайтесь', text: 'Создайте main и выведите любой текст через println.', starter: 'fun main() {\n    // Напишите println здесь\n}', check: (code: string) => /fun\s+main[\s\S]*println\s*\(/.test(code) },
  { title: 'Переменная и тип', text: 'Сохраните число в переменной int и выведите его.', starter: 'fun main() {\n    int score = 10\n    println(score)\n}', check: (code: string) => /int\s+\w+\s*=/.test(code) && /println\s*\(/.test(code) },
  { title: 'Своя функция', text: 'Создайте функцию с параметром и вызовите её из main.', starter: 'fun greet(name: text) {\n    println("Привет, " + name)\n}\n\nfun main() {\n    greet("Фу")\n}', check: (code: string) => /fun\s+(?!main)\w+\s*\([^)]*\w+\s*:\s*\w+/.test(code) && /fun\s+main/.test(code) },
  { title: 'Цикл и условие', text: 'Пройдите циклом по диапазону и добавьте if.', starter: 'fun main() {\n    for i in 1..5 {\n        if i > 2 {\n            println(i)\n        }\n    }\n}', check: (code: string) => /for\s+\w+\s+in/.test(code) && /if\s+/.test(code) },
  { title: 'Мини-проект', text: 'Соберите программу из двух функций, коллекции и цикла. Затем запустите её.', starter: 'fun showNames(names: list<text>) {\n    for name in names {\n        println(name)\n    }\n}\n\nfun main() {\n    list<text> names = ["Alex", "Steve"]\n    showNames(names)\n}', check: (code: string) => (code.match(/fun\s+\w+/g)?.length || 0) >= 2 && /list\s*</.test(code) && /for\s+/.test(code) }
];

function renderLessons() {
  const step = Math.min(settings.tutorial_step, learningTasks.length - 1);
  const task = learningTasks[step];
  const completed = settings.tutorial_step >= learningTasks.length;
  showSurface(`<div class="surface-page learning-page"><div class="page-hero"><div><span class="overline">ПУТЬ НОВИЧКА</span><h1>${completed ? 'Все ступени пройдены!' : `Шаг ${step + 1}: ${task.title}`}</h1><p>${completed ? 'Теперь попробуйте собственный проект или Minecraft-мод.' : task.text}</p></div><div class="learning-progress"><b>${Math.min(settings.tutorial_step, learningTasks.length)}/${learningTasks.length}</b><span>ступеней</span></div></div><div class="lesson-track">${learningTasks.map((item, index) => `<div class="lesson-step ${index < settings.tutorial_step ? 'done' : index === step && !completed ? 'active' : ''}"><span>${index < settings.tutorial_step ? '✓' : index + 1}</span><b>${item.title}</b></div>`).join('')}</div><section class="lesson-work"><div><h2>${completed ? 'Что дальше?' : 'Ваше задание'}</h2><p>${completed ? 'Откройте библиотеки, создайте свой плагин или независимую Minecraft-сборку.' : task.text}</p>${completed ? '<button class="primary" id="restartLearning">Пройти ещё раз</button>' : `<div class="modal-actions"><button class="secondary" id="lessonStarter">Открыть заготовку</button><button class="primary" id="checkLesson">Проверить мой код</button></div>`}</div><div class="lesson-hint">${icon('spark')}<p><b>Не бойтесь ошибок.</b><br>Помощник объяснит их простыми словами, а исходный код всегда сохраняется автоматически.</p></div></section></div>`);
  if (completed) { document.getElementById('restartLearning')!.onclick = () => { settings.tutorial_step = 0; void saveSettings(settings); renderLessons(); }; return; }
  document.getElementById('lessonStarter')!.onclick = async () => {
    const path = 'learning.fun'; await saveFile(project.root, path, task.starter); project = await reloadProject(project.root); renderExplorer(); openFile(path); toast('Заготовка открыта. Измените её и вернитесь к пути новичка.');
  };
  document.getElementById('checkLesson')!.onclick = () => {
    const code = editor.getValue();
    if (!task.check(code)) { toast('Пока не все условия выполнены. Сверьтесь с заданием и попробуйте ещё раз.', 'warn'); return; }
    settings.tutorial_step += 1; void saveSettings(settings); toast('Ступень пройдена! Отличная работа.'); renderLessons();
  };
}

async function renderSettings() {
  const desktopSections = `<section><h2>Компилятор</h2><label><span><b>Backend по умолчанию</b><small>JVM, C++ 17, Rust, JavaScript или Python</small></span><select id="defaultBackend"><option value="jvm">JVM / Java</option><option value="cpp">C++ 17</option><option value="rust">Rust</option><option value="javascript">JavaScript</option><option value="python">Python</option></select></label><div class="cli-card"><div>${icon('terminal')}</div><span><b>Funo CLI и PATH</b><small id="pathDescription">Проверяю пользовательский PATH…</small><code id="pathLauncher"></code></span><button class="primary" id="togglePath" disabled>Проверка…</button></div></section><section><h2>Minecraft и Microsoft</h2><label><span><b>Microsoft Entra Client ID</b><small>Public client с разрешённым device-code flow. Секрет не нужен и не хранится.</small></span><input id="microsoftClientId" value="${escapeHtml(settings.microsoft_client_id)}" placeholder="00000000-0000-0000-0000-000000000000"></label><label><span><b>Аккаунт</b><small>${account ? `Подключён: ${escapeHtml(account.username)}` : 'Не подключён'}</small></span><button class="secondary" id="settingsAccount">${account ? 'Управление' : 'Войти'}</button></label><div class="cli-card compact"><div>${icon('java')}</div><span><b>JDK и Gradle для Minecraft</b><small>Проверка версий, обновления и установка с обязательным резервом 30 ГиБ</small></span><button class="secondary" id="settingsToolchains">Открыть</button></div></section>`;
  const mobileSections = `<section><h2>Android APK</h2><div class="mobile-notice settings-mobile"><b>Локальная мобильная Studio</b><span>Обычный Funo работает без JDK. Портативная Java нужна только для локальной сборки модов и Minecraft Java.</span></div><div class="mobile-capability-grid"><article>${icon('play')}<span><b>Встроено</b><small>Редактор, запуск Funo, Gradle-сборка Minecraft, Launcher, вики и каталоги</small></span></article><article>${icon('java')}<span><b>Портативная Java</b><small>Android JDK и совместимый Gradle с проверкой SHA-256 и резервом 30 ГиБ</small><button class="secondary" id="settingsToolchains">Установить портативную Java</button></span></article></div></section>`;
  showSurface(`<div class="surface-page settings-page"><div class="page-hero"><div><span class="overline">НАСТРОЙКИ</span><h1>Funo Studio</h1><p>Редактор хранит исходники локально. Код не отправляется в облако.</p></div>${runtimePlatform.android ? '<span class="platform-pill">Android</span>' : ''}</div><div class="settings-list"><section><h2>Редактор и обучение</h2><label><span><b>Режим по умолчанию</b><small>В режиме новичка подсказки подробнее</small></span><select id="defaultMode"><option value="novice">Я новичок</option><option value="pro">Профессиональный</option></select></label><label><span><b>Интерактивное обучение</b><small>Продолжить с сохранённого шага</small></span><button class="secondary" id="openLearning">Открыть путь</button></label></section>${desktopToolsAvailable ? desktopSections : mobileSections}<section><h2>Пакеты</h2><label><span><b>Официальный GitHub</b><small>Индекс проверенных библиотек${desktopToolsAvailable ? '' : ' · просмотр на Android'}</small></span><input value="vanyachickenganidanya-lgtm/funo_libsOFFICAL" readonly></label>${desktopToolsAvailable ? `<div class="cli-card compact"><div>${icon('package')}</div><span><b>Свои плагины</b><small>Rust, C++ 17, TypeScript, JavaScript и Python</small></span><button class="secondary" id="settingsPlugins">Открыть SDK</button></div>` : ''}</section></div></div>`);
  const modeSelect = document.getElementById('defaultMode') as HTMLSelectElement;
  modeSelect.value = settings.beginner ? 'novice' : 'pro';
  modeSelect.onchange = () => setMode(modeSelect.value as 'novice' | 'pro');
  document.getElementById('openLearning')!.onclick = () => { selectView('lessons'); renderLessons(); };
  if (!desktopToolsAvailable) {
    document.getElementById('settingsToolchains')!.onclick = () => {
      const requirements = minecraftRequirements();
      void renderMinecraftToolchains(project.root, requirements.version, requirements.loader, undefined, false, 'settings');
    };
    return;
  }

  const backendSelect = document.getElementById('defaultBackend') as HTMLSelectElement;
  backendSelect.value = compilerBackend;
  backendSelect.onchange = () => { compilerBackend = backendSelect.value; settings.compiler_backend = compilerBackend; document.getElementById('backendStatus')!.textContent = compilerBackend.toUpperCase(); void saveSettings(settings); };
  (document.getElementById('microsoftClientId') as HTMLInputElement).onchange = event => { settings.microsoft_client_id = (event.target as HTMLInputElement).value.trim(); void saveSettings(settings); };
  document.getElementById('settingsAccount')!.onclick = () => void handleMicrosoftAccount();
  document.getElementById('settingsToolchains')!.onclick = () => {
    const requirements = minecraftRequirements();
    void renderMinecraftToolchains(project.root, requirements.version, requirements.loader, undefined, false, 'settings');
  };
  document.getElementById('settingsPlugins')!.onclick = () => void renderPlugins();
  try {
    const status = await getPathStatus();
    const button = document.getElementById('togglePath') as HTMLButtonElement;
    document.getElementById('pathDescription')!.textContent = status.installed ? 'Команда funo доступна в новых терминалах.' : 'Добавьте команду funo в пользовательский PATH без прав администратора.';
    document.getElementById('pathLauncher')!.textContent = status.launcher;
    button.disabled = false; button.textContent = status.installed ? 'Убрать из PATH' : 'Добавить в PATH'; button.className = status.installed ? 'secondary' : 'primary';
    button.onclick = async () => { button.disabled = true; try { if (status.installed) await uninstallPath(); else await installPath(); toast(status.installed ? 'Funo удалён из PATH.' : 'Funo добавлен в PATH. Откройте новый терминал.'); await renderSettings(); } catch (error) { toast(String(error), 'warn'); button.disabled = false; } };
  } catch (error) { document.getElementById('pathDescription')!.textContent = String(error); }
}

function showSurface(html: string) {
  const surface = document.getElementById('surface')!; surface.innerHTML = html; surface.classList.remove('hidden');
}
function hideSurface() { const s = document.getElementById('surface')!; s.classList.add('hidden'); s.innerHTML = ''; }

function selectView(view: string) {
  const closeCurrentDrawer = compactLayout() && currentView === view && document.body.classList.contains('sidebar-open');
  currentView = view;
  document.querySelectorAll<HTMLElement>('.activity').forEach(x => x.classList.toggle('active', x.dataset.view === view));
  const title = document.getElementById('sidebarTitle')!;
  document.getElementById('newFile')!.classList.toggle('hidden', view !== 'explorer');
  document.getElementById('newFolder')!.classList.toggle('hidden', view !== 'explorer');
  if (view === 'explorer') { title.textContent = 'ПРОВОДНИК'; renderExplorer(); hideSurface(); }
  else if (view === 'search') { title.textContent = 'ПОИСК'; renderSearch(); hideSurface(); }
  else if (view === 'run') { title.textContent = 'ЗАПУСК И ОТЛАДКА'; renderRunSide(); hideSurface(); }
  else if (view === 'packages') { title.textContent = 'БИБЛИОТЕКИ'; document.getElementById('sidebarContent')!.innerHTML = `<div class="side-info">${icon('package')}<b>Funo Pack</b><p>Официальный реестр подключён к вашему GitHub.</p></div>`; void renderPackages(); }
  else if (view === 'minecraft') { title.textContent = 'MINECRAFT'; document.getElementById('sidebarContent')!.innerHTML = `<div class="side-info">${icon('cube')}<b>Создание модов</b><p>Fabric, Forge и NeoForge с простым кодом Funo.</p></div>`; renderMinecraft(); }
  else if (view === 'plugins') { title.textContent = 'ПЛАГИНЫ'; document.getElementById('sidebarContent')!.innerHTML = `<div class="side-info">${icon('branch')}<b>Plugin SDK</b><p>Собственные Rust, C++, TypeScript, JavaScript и Python расширения.</p></div>`; void renderPlugins(); }
  else if (view === 'lessons') { title.textContent = 'ПУТЬ НОВИЧКА'; document.getElementById('sidebarContent')!.innerHTML = `<div class="side-info">${icon('spark')}<b>Учимся практикой</b><p>Пять программ от первого вывода до мини-проекта.</p></div>`; renderLessons(); }
  else if (view === 'wiki') { title.textContent = 'ВИКИ FUNO'; document.getElementById('sidebarContent')!.innerHTML = `<div class="side-info">${icon('book')}<b>Документация</b><p>От первой функции до своего пакета.</p></div>`; renderWiki(); }
  else if (view === 'settings') { title.textContent = 'УПРАВЛЕНИЕ'; document.getElementById('sidebarContent')!.innerHTML = ''; void renderSettings(); }
  if (compactLayout()) {
    const drawerView = view === 'explorer' || view === 'search' || view === 'run';
    document.body.classList.toggle('sidebar-open', drawerView && !closeCurrentDrawer);
  }
}

document.querySelectorAll<HTMLElement>('.activity').forEach(x => x.onclick = () => selectView(x.dataset.view!));
document.getElementById('sidebarClose')!.onclick = () => document.body.classList.remove('sidebar-open');
document.getElementById('drawerScrim')!.onclick = () => document.body.classList.remove('sidebar-open');
window.matchMedia('(max-width: 650px)').addEventListener('change', event => {
  if (!event.matches) document.body.classList.remove('sidebar-open', 'panel-collapsed');
  editor.layout();
});

document.getElementById('newFile')!.onclick = async () => {
  const name = prompt('Путь нового файла', 'src/feature.fun')?.trim(); if (!name) return;
  if (name.startsWith('/') || name.split('/').some(part => !part || part === '..' || part === '.')) return toast('Введите безопасный путь внутри проекта.', 'warn');
  if (project.files.some(file => file.path === name)) return toast('Такой файл уже существует.', 'warn');
  const content = name.endsWith('.fun') ? 'fun hello() {\n    println("Новая функция готова!")\n}' : name.endsWith('.md') ? '# Новый документ\n\nНачните писать здесь.' : '';
  await saveFile(project.root, name, content);
  project = await reloadProject(project.root);
  renderExplorer(); openFile(name);
};

document.getElementById('newFolder')!.onclick = async () => {
  const path = prompt('Путь новой папки', 'src/features')?.trim(); if (!path) return;
  try { project = await createFolder(project.root, path); renderExplorer(); toast(`Папка ${path} создана.`); }
  catch (error) { toast(String(error), 'warn'); }
};

document.getElementById('refreshView')!.onclick = async () => {
  try { project = await reloadProject(project.root); selectView(currentView); }
  catch (error) { toast(String(error), 'warn'); }
};

editor.onDidChangeModelContent(() => {
  if (!editor.getModel()) return;
  (window as any).__lastJava = '';
  const file = project?.files.find(f => f.path === currentPath); if (file) file.content = editor.getValue();
  updateMarkdownPreview();
  document.getElementById('dirtyDot')!.classList.add('on');
  document.getElementById('syncStatus')!.innerHTML = '◌ сохранение…';
  window.clearTimeout(saveTimer); saveTimer = window.setTimeout(async () => {
    try { await saveFile(project.root, currentPath, editor.getValue()); document.getElementById('dirtyDot')!.classList.remove('on'); document.getElementById('syncStatus')!.innerHTML = `${icon('check')} сохранено`; }
    catch (err) { document.getElementById('syncStatus')!.textContent = 'не сохранено'; toast(String(err), 'warn'); }
  }, 450);
  window.clearTimeout(checkTimer); checkTimer = window.setTimeout(() => void diagnose(), 260);
});

editor.onDidChangeCursorPosition(e => { document.getElementById('cursorStatus')!.textContent = `Стр ${e.position.lineNumber}, Стлб ${e.position.column}`; });

async function diagnose() {
  const model = editor.getModel(); if (!model || !currentPath.endsWith('.fun')) { diagnostics = []; return; }
  diagnostics = await checkCode(model.getValue()).catch(() => []);
  setDiagnostics(model, diagnostics);
  const errors = diagnostics.filter(d => d.severity === 'error').length;
  const warns = diagnostics.filter(d => d.severity === 'warning').length;
  document.getElementById('errorStatus')!.textContent = `× ${errors}   △ ${warns}`;
  document.getElementById('problemCount')!.textContent = String(diagnostics.length);
  if (diagnostics.length) renderFriendlyError(diagnostics[0]); else document.getElementById('friendlyCard')!.classList.add('hidden');
  if (currentPanel === 'problems') renderPanel();
}

function renderFriendlyError(d: Diagnostic) {
  const card = document.getElementById('friendlyCard')!;
  card.classList.remove('hidden');
  card.innerHTML = `<button class="card-close">${icon('close')}</button><div class="friend-icon">${icon('spark')}</div><div class="friend-body"><small>${mode === 'novice' ? 'ПОМОЩНИК ФУ' : d.code}</small><b>${d.title}</b><p>${mode === 'novice' ? d.message : `${d.code} · строка ${d.line}:${d.column}`}</p>${d.example ? `<pre>${escapeHtml(d.example)}</pre>` : ''}<div><button class="primary small" id="applyFix">Исправить автоматически</button><button class="secondary small" id="goProblem">Показать место</button></div></div>`;
  card.querySelector<HTMLElement>('.card-close')!.onclick = () => card.classList.add('hidden');
  document.getElementById('goProblem')!.onclick = () => { editor.revealLineInCenter(d.line); editor.setPosition({ lineNumber: d.line, column: d.column }); editor.focus(); };
  document.getElementById('applyFix')!.onclick = () => applyFix(d);
}

function applyFix(d: Diagnostic) {
  const model = editor.getModel(); if (!model || !d.replacement) return;
  if (d.code === 'FUN002') {
    editor.executeEdits('funo-auto-fix', [{ range: new monaco.Range(model.getLineCount(), model.getLineMaxColumn(model.getLineCount()), model.getLineCount(), model.getLineMaxColumn(model.getLineCount())), text: d.replacement }]);
  } else {
    editor.executeEdits('funo-auto-fix', [{ range: new monaco.Range(d.line, d.column, d.line, d.end_column), text: d.replacement }]);
  }
  toast('Исправлено. Ничего страшного — продолжаем!'); editor.focus(); void diagnose();
}

function escapeHtml(s: string) { return s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!)); }

async function execute() {
  selectPanel('terminal');
  if (compactLayout()) document.body.classList.remove('panel-collapsed');
  const panel = document.getElementById('panelBody')!;
  const minecraft = project.kind.startsWith('minecraft');
  const mobileAction = minecraft ? 'funo minecraft build · встроенный Gradle' : 'funo run · встроенный интерпретатор';
  panel.innerHTML = `<span class="muted">› ${desktopToolsAvailable ? 'funo run' : mobileAction}\n  ${!desktopToolsAvailable && !minecraft ? 'Запускаю обычную Funo-программу без JDK…' : 'Проверяю инструменты и готовлю локальную сборку…'}</span>`;
  try {
    if (!desktopToolsAvailable && !minecraft) {
      // Ordinary Android projects continue to use the bounded, process-free
      // Rust interpreter and do not pay the cost of launching a JVM.
      const result = await runCode(project.root, editor.getValue());
      diagnostics = result.diagnostics;
      setDiagnostics(editor.getModel()!, diagnostics);
      document.getElementById('problemCount')!.textContent = String(diagnostics.length);
      document.getElementById('errorStatus')!.textContent = `× ${diagnostics.filter(item => item.severity === 'error').length}   △ ${diagnostics.filter(item => item.severity === 'warning').length}`;
      if (result.success) {
        (window as any).__lastJava = result.generated_java;
        panel.innerHTML = `${escapeHtml(result.stdout)}${result.stdout ? '\n' : ''}<span class="success">✓ Выполнено на устройстве</span>\n<span class="muted">Встроенный Funo-интерпретатор · ${result.elapsed_ms} мс · без JDK</span>`;
        toast('Funo-программа выполнена на устройстве.');
      } else {
        panel.innerHTML = `<span class="error">Выполнение остановлено</span>\n${escapeHtml(result.stderr)}${result.stdout ? `\n<span class="muted">Вывод до остановки:</span>\n${escapeHtml(result.stdout)}` : ''}`;
        if (diagnostics.length) renderFriendlyError(diagnostics[0]);
      }
      return;
    }
    const native = !minecraft && compilerBackend !== 'jvm';
    const shouldRun = (document.getElementById('backendMode') as HTMLSelectElement | null)?.value !== 'build';
    if (minecraft) {
      const requirements = minecraftRequirements();
      const tools = await minecraftToolchainStatus(project.root, requirements.version, requirements.loader);
      if (!tools.ready) {
        panel.innerHTML = `<span class="error">Нужны JDK ${tools.required_java} и совместимый Gradle.</span>\n${escapeHtml(tools.message)}\n<span class="muted">Открыт безопасный установщик с резервом 30 ГиБ.</span>`;
        toast('Сначала установите инструменты Minecraft.', 'warn');
        await renderMinecraftToolchains(project.root, requirements.version, requirements.loader);
        return;
      }
    }
    const result = native ? await runBackend(project.root, editor.getValue(), compilerBackend, shouldRun) : minecraft ? await buildMinecraft(project.root, editor.getValue()) : await runCode(project.root, editor.getValue());
    if (result.success) panel.innerHTML = `${escapeHtml(result.stdout)}${result.stdout ? '\n' : ''}<span class="success">✓ ${minecraft ? 'Minecraft-мод собран' : native ? `${compilerBackend.toUpperCase()} готов` : `Завершено успешно${/return\s*\(\s*200/.test(editor.getValue()) ? ' · Funo 200' : ''}`}</span>\n<span class="muted">${minecraft ? 'Gradle' : compilerBackend.toUpperCase()} · ${result.elapsed_ms} мс${result.artifact ? ` · ${escapeHtml(result.artifact)}` : ''}</span>`;
    else panel.innerHTML = `<span class="error">Сборка остановлена</span>\n${escapeHtml(result.stderr)}\n<span class="muted">Помощник покажет, как исправить код.</span>`;
    if ('generated_java' in result) (window as any).__lastJava = result.generated_java;
    if (result.diagnostics.length) { diagnostics = result.diagnostics; renderFriendlyError(diagnostics[0]); }
  } catch (err) { panel.innerHTML = `<span class="error">Не удалось запустить</span>\n${escapeHtml(String(err))}`; }
}

document.getElementById('topRun')!.onclick = () => void execute();
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => void execute());
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => void saveFile(project.root, currentPath, editor.getValue()).then(() => toast('Файл сохранён.')));

document.getElementById('showJava')!.onclick = async () => {
  let java = (window as any).__lastJava;
  if (!java) {
    const result = desktopToolsAvailable
      ? await runCode(project.root, editor.getValue())
      : await transpileSource(editor.getValue(), project.kind.startsWith('minecraft'));
    if (!result.success) { toast(result.stderr || 'Сначала исправьте ошибки Funo.', 'warn'); return; }
    java = result.generated_java; (window as any).__lastJava = java;
  }
  openModal('Java, созданный компилятором Funo', `<p>${desktopToolsAvailable ? 'Этот код передаётся в <code>javac</code>.' : 'Android показывает результат встроенного компилятора без запуска <code>javac</code>.'} Исходный .fun остаётся главным файлом.</p><pre class="java-preview">${escapeHtml(java || 'Java-код пока недоступен.')}</pre><div class="modal-actions"><button class="primary" data-close>Готово</button></div>`);
};

function selectPanel(panel: 'terminal' | 'problems' | 'output') {
  currentPanel = panel;
  if (compactLayout()) document.body.classList.remove('panel-collapsed');
  document.querySelectorAll<HTMLElement>('.panel-tab').forEach(x => x.classList.toggle('active', x.dataset.panel === panel));
  renderPanel();
}
document.querySelectorAll<HTMLElement>('.panel-tab').forEach(x => x.onclick = () => selectPanel(x.dataset.panel as any));
document.getElementById('collapsePanel')!.onclick = () => {
  document.body.classList.toggle('panel-collapsed');
  window.setTimeout(() => editor.layout(), 180);
};
function renderPanel() {
  const body = document.getElementById('panelBody')!;
  if (currentPanel === 'problems') body.innerHTML = diagnostics.length ? diagnostics.map(d => `<button class="problem-line" data-line="${d.line}"><span class="${d.severity}">●</span> ${escapeHtml(d.title)} <em>[${d.code}] строка ${d.line}</em></button>`).join('') : '<span class="muted">Проблем не обнаружено.</span>';
  else if (currentPanel === 'output') body.innerHTML = `<span class="muted">Funo Language Server\nОфициальный реестр: vanyachickenganidanya-lgtm/funo_libsOFFICAL\n${desktopToolsAvailable ? 'JVM backend: готов' : 'Android: встроенный запуск Funo, проверка и Java-предпросмотр'}</span>`;
  document.querySelectorAll<HTMLElement>('.problem-line').forEach(x => x.onclick = () => { editor.revealLineInCenter(Number(x.dataset.line)); editor.setPosition({ lineNumber: Number(x.dataset.line), column: 1 }); editor.focus(); });
}
document.getElementById('clearPanel')!.onclick = () => document.getElementById('panelBody')!.innerHTML = '';

function openModal(title: string, body: string) {
  const layer = document.getElementById('modalLayer')!;
  document.getElementById('modal')!.classList.remove('onboarding-modal');
  document.getElementById('modal')!.innerHTML = `<header><h2>${title}</h2><button data-close>${icon('close')}</button></header><main>${body}</main>`;
  layer.classList.remove('hidden');
  layer.querySelectorAll<HTMLElement>('[data-close]').forEach(x => x.onclick = () => layer.classList.add('hidden'));
}
document.getElementById('modalLayer')!.onclick = e => { if (e.target === document.getElementById('modalLayer')) document.getElementById('modalLayer')!.classList.add('hidden'); };

function updateProjectUI() {
  document.querySelector('.command-center span')!.textContent = project.name;
  document.getElementById('projectCrumb')!.textContent = project.name;
  renderExplorer();
}

async function showOnboarding() {
  const path = desktopToolsAvailable
    ? await getPathStatus().catch(() => ({ installed: false, path_contains_bin: false, bin_dir: '', launcher: 'funo' }))
    : { installed: false, path_contains_bin: false, bin_dir: '', launcher: 'funo' };
  const beginnerDefault = settings.installer_beginner_choice ?? settings.beginner;
  const pathStep = desktopToolsAvailable ? `<section><h3>2. Команда в терминале</h3><label class="choice-card"><input type="checkbox" id="onboardingPath" ${path.installed ? 'checked disabled' : 'checked'}><span><b>${path.installed ? 'Funo уже добавлен в PATH' : 'Добавить Funo в пользовательский PATH'}</b><small>Команда <code>funo</code> станет доступна в новых терминалах. Права администратора не нужны.</small></span></label></section>` : `<section class="mobile-welcome"><h3>Android-режим</h3><p>Исходники хранятся на устройстве. Здесь доступны редактор, настоящий запуск обычного Funo без JDK, проверка, Java-предпросмотр, обучение и каталоги.</p></section>`;
  openModal('Добро пожаловать в Funo Studio', `<div class="onboarding"><div class="onboarding-mark">F</div><p>Настроим Studio перед первым проектом. Режим обучения позже можно изменить в настройках.</p><section><h3>1. Как вы хотите учиться?</h3><label class="choice-card"><input type="radio" name="experience" value="novice" ${beginnerDefault ? 'checked' : ''}><span><b>Я новичок</b><small>Больше объяснений и интерактивный путь из пяти программ</small></span></label><label class="choice-card"><input type="radio" name="experience" value="pro" ${beginnerDefault ? '' : 'checked'}><span><b>Профессиональный режим</b><small>Компактные ошибки и меньше подсказок</small></span></label></section>${pathStep}<div class="modal-actions"><button class="primary big" id="finishOnboarding">Начать работу</button></div></div>`);
  document.getElementById('modal')!.classList.add('onboarding-modal');
  document.querySelector<HTMLElement>('#modal > header > button')!.classList.add('hidden');
  document.getElementById('finishOnboarding')!.onclick = async () => {
    const button = document.getElementById('finishOnboarding') as HTMLButtonElement; button.disabled = true; button.textContent = 'Настраиваю…';
    const novice = (document.querySelector<HTMLInputElement>('input[name="experience"]:checked')?.value || 'novice') === 'novice';
    settings.beginner = novice; settings.onboarding_completed = true; settings.compiler_backend = compilerBackend;
    await saveSettings(settings); setMode(novice ? 'novice' : 'pro');
    const pathChoice = document.getElementById('onboardingPath') as HTMLInputElement | null;
    if (desktopToolsAvailable && !path.installed && pathChoice?.checked) {
      try { await installPath(); toast('Команда funo добавлена в PATH. Откройте новый терминал.'); }
      catch (error) { toast(`PATH можно настроить позже: ${String(error)}`, 'warn'); }
    }
    document.getElementById('modalLayer')!.classList.add('hidden');
    if (novice) { selectView('lessons'); renderLessons(); }
  };
}

document.getElementById('commandCenter')!.onclick = () => {
  openModal('Команды Funo', `<div class="command-list"><button data-command="run">${icon('play')} Запустить текущий файл <kbd>Ctrl Enter</kbd></button><button data-command="packages">${icon('package')} Открыть библиотеки</button><button data-command="minecraft">${icon('cube')} Создать Minecraft-мод</button><button data-command="wiki">${icon('book')} Открыть вики</button></div>`);
  document.querySelectorAll<HTMLElement>('[data-command]').forEach(button => button.onclick = () => {
    document.getElementById('modalLayer')!.classList.add('hidden');
    const command = button.dataset.command!;
    if (command === 'run') void execute();
    else selectView(command);
  });
};

async function init() {
  try {
    settings = await loadSettings();
    compilerBackend = settings.compiler_backend || 'jvm';
    mode = settings.beginner ? 'novice' : 'pro';
    account = await currentMicrosoftAccount().catch(() => null);
    project = await ensureProject(); updateProjectUI(); openFile(project.files.find(f => f.path === 'main.fun')?.path || project.files[0].path); setMode(mode);
    document.getElementById('backendStatus')!.textContent = compilerBackend.toUpperCase();
    if (!settings.onboarding_completed) await showOnboarding();
  } catch (err) {
    document.getElementById('panelBody')!.innerHTML = `<span class="error">Не удалось открыть проект: ${escapeHtml(String(err))}</span>`;
  }
}
void init();
