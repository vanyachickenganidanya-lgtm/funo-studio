import './styles.css';
import * as monaco from 'monaco-editor';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import { registerFunoLanguage, setDiagnostics } from './funo-language';
import {
  ensureProject, saveFile, checkCode, runCode, buildMinecraft, fetchRegistry, installPackage,
  createMinecraftProject, type Project, type Diagnostic, type RegistryPackage
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

const app = document.querySelector<HTMLDivElement>('#app')!;
app.innerHTML = `
<div class="shell">
  <header class="titlebar" data-tauri-drag-region>
    <div class="app-mark">f;</div>
    <nav class="menu"><button>Файл</button><button>Правка</button><button>Выделение</button><button>Вид</button><button>Запуск</button><button>Помощь</button></nav>
    <button class="command-center" id="commandCenter">${icon('search')} <span>Мой первый проект</span><kbd>Ctrl K</kbd></button>
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
      <div class="sidebar-title"><span id="sidebarTitle">ПРОВОДНИК</span><div><button class="tiny" id="newFile" title="Новый файл">${icon('plus')}</button><button class="tiny" id="refreshView">${icon('refresh')}</button></div></div>
      <div class="sidebar-content" id="sidebarContent"></div>
    </aside>
    <section class="editor-group">
      <div class="editor-tabs"><button class="editor-tab active"><span class="fun-icon">fn</span><span id="tabTitle">main.fun</span><span class="dirty" id="dirtyDot"></span>${icon('close')}</button><div class="editor-actions"><button id="showJava" title="Показать Java">${icon('java')}</button><button id="topRun" title="Запустить Ctrl+Enter">${icon('play')}</button></div></div>
      <div class="breadcrumbs"><span id="projectCrumb">Мой первый проект</span><b>›</b><span id="fileCrumb">main.fun</span><b>›</b><span id="symbolCrumb">fun main()</span></div>
      <div class="editor-wrap">
        <div id="editor"></div>
        <div class="friendly-card hidden" id="friendlyCard"></div>
        <div class="surface hidden" id="surface"></div>
      </div>
      <div class="panel">
        <div class="panel-head">
          <button class="panel-tab active" data-panel="terminal">ТЕРМИНАЛ</button>
          <button class="panel-tab" data-panel="problems">ПРОБЛЕМЫ <span id="problemCount">0</span></button>
          <button class="panel-tab" data-panel="output">ВЫХОД</button>
          <div class="panel-actions"><button id="clearPanel">${icon('close')}</button></div>
        </div>
        <pre class="panel-body" id="panelBody"><span class="muted">Funo готов. Нажмите Ctrl+Enter, чтобы запустить программу.</span></pre>
      </div>
    </section>
  </div>
  <footer class="statusbar">
    <span>${icon('branch')} main*</span><span id="syncStatus">${icon('check')} сохранено</span>
    <span id="errorStatus">× 0&nbsp;&nbsp; △ 0</span>
    <span class="status-spacer"></span><span id="cursorStatus">Стр 1, Стлб 1</span><span>Пробелы: 4</span><span>UTF-8</span><span>{ } Funo</span><span>JVM</span>
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

const editor = monaco.editor.create(document.getElementById('editor')!, {
  value: '', language: 'funo', theme: 'funo-vscode', automaticLayout: true,
  fontFamily: '"Cascadia Code", "JetBrains Mono", Consolas, monospace', fontSize: 14,
  lineHeight: 22, fontLigatures: true, minimap: { enabled: true, scale: 1 },
  smoothScrolling: true, cursorSmoothCaretAnimation: 'on', padding: { top: 10 },
  renderWhitespace: 'selection', bracketPairColorization: { enabled: true },
  guides: { bracketPairs: true, indentation: true }, stickyScroll: { enabled: true },
  suggest: { showWords: false, preview: true }, quickSuggestions: { other: true, comments: false, strings: false },
  inlineSuggest: { enabled: true }, lightbulb: { enabled: monaco.editor.ShowLightbulbIconMode.OnCode },
  wordWrap: 'off', tabSize: 4, insertSpaces: true
});

let project: Project;
let currentPath = 'main.fun';
let diagnostics: Diagnostic[] = [];
let currentPanel: 'terminal' | 'problems' | 'output' = 'terminal';
let currentView = 'explorer';
let saveTimer = 0;
let checkTimer = 0;
let mode: 'novice' | 'pro' = (localStorage.getItem('funo-mode') as any) || 'novice';
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
  editor.updateOptions({ minimap: { enabled: next === 'pro' }, inlayHints: { enabled: next === 'pro' ? 'on' : 'off' } });
  if (diagnostics.length) renderFriendlyError(diagnostics[0]);
}

document.getElementById('noviceMode')!.onclick = () => setMode('novice');
document.getElementById('proMode')!.onclick = () => setMode('pro');

function languageFor(path: string) { return path.endsWith('.fun') ? 'funo' : path.endsWith('.json') ? 'json' : path.endsWith('.toml') ? 'ini' : 'plaintext'; }

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
  document.getElementById('symbolCrumb')!.textContent = path.endsWith('.fun') ? 'Funo' : 'Настройки';
  document.querySelectorAll('.file-row').forEach(x => x.classList.toggle('active', (x as HTMLElement).dataset.path === path));
  hideSurface();
  void diagnose();
}

function fileIcon(path: string) { return path.endsWith('.fun') ? '<span class="file-type fun">fn</span>' : path.endsWith('.toml') ? '<span class="file-type toml">⚙</span>' : '<span class="file-type">·</span>'; }

function renderExplorer() {
  const root = project.files.filter(f => !f.path.includes('/'));
  const nested = project.files.filter(f => f.path.includes('/'));
  document.getElementById('sidebarContent')!.innerHTML = `
    <div class="tree-title"><span>⌄</span><b>${project.name.toUpperCase()}</b></div>
    <div class="file-tree">
      ${root.map(f => `<button class="file-row ${f.path === currentPath ? 'active' : ''}" data-path="${f.path}">${fileIcon(f.path)}<span>${f.path}</span></button>`).join('')}
      ${nested.length ? `<div class="folder-row"><span>⌄</span><span>src</span></div>${nested.map(f => `<button class="file-row nested ${f.path === currentPath ? 'active' : ''}" data-path="${f.path}">${fileIcon(f.path)}<span>${f.path.split('/').pop()}</span></button>`).join('')}` : ''}
    </div>
    <div class="outline"><div class="section-line">⌄ СТРУКТУРА</div><button><span class="method-dot">◇</span> fib(n) <em>int</em></button><button><span class="method-dot">◇</span> main() <em>void</em></button></div>`;
  document.querySelectorAll<HTMLElement>('.file-row').forEach(el => el.onclick = () => openFile(el.dataset.path!));
}

function renderSearch() {
  document.getElementById('sidebarContent')!.innerHTML = `<div class="search-side"><input id="globalSearch" placeholder="Поиск" autofocus><button class="primary small" id="doSearch">Найти в проекте</button><p class="side-help">Поиск работает по всем .fun-файлам проекта.</p><div id="searchResults"></div></div>`;
  const run = () => {
    const q = (document.getElementById('globalSearch') as HTMLInputElement).value;
    const hits = project.files.flatMap(f => f.content.split('\n').map((line, i) => ({ f, line, i })).filter(x => q && x.line.toLowerCase().includes(q.toLowerCase())));
    document.getElementById('searchResults')!.innerHTML = hits.map(h => `<button class="search-hit" data-path="${h.f.path}" data-line="${h.i + 1}"><b>${h.f.path}:${h.i + 1}</b><span>${h.line.trim()}</span></button>`).join('') || '<p class="side-help">Совпадений пока нет.</p>';
    document.querySelectorAll<HTMLElement>('.search-hit').forEach(x => x.onclick = () => { openFile(x.dataset.path!); editor.revealLineInCenter(Number(x.dataset.line)); editor.setPosition({ lineNumber: Number(x.dataset.line), column: 1 }); });
  };
  document.getElementById('doSearch')!.onclick = run;
  (document.getElementById('globalSearch') as HTMLInputElement).onkeydown = e => { if (e.key === 'Enter') run(); };
}

function renderRunSide() {
  document.getElementById('sidebarContent')!.innerHTML = `<div class="run-side"><button class="primary full" id="runSideButton">${icon('play')} Запустить Funo</button><p class="side-help">Компилятор создаст Java, вызовет javac и запустит программу локально.</p><div class="side-section-title">КОНФИГУРАЦИЯ</div><label>Цель<select><option>JVM / Java</option><option>Minecraft Fabric</option><option>Minecraft Forge</option></select></label><label>JDK<select><option>Автоматически</option><option>Java 21</option><option>Java 17</option></select></label><label class="check"><input type="checkbox" checked> Остановиться при ошибке</label></div>`;
  document.getElementById('runSideButton')!.onclick = () => void execute();
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
    <div class="page-hero"><div><span class="overline">FUNO PACK</span><h1>Библиотеки</h1><p>Проверенные пакеты из вашего GitHub, Java .jar и инструменты для Minecraft.</p></div><button class="secondary" id="refreshRegistry">${icon('refresh')} Обновить</button></div>
    <div class="registry-source"><span class="verified-mark">${icon('check')}</span><div><b>Официальный источник</b><code>${result.source}</code></div><span class="registry-state ${result.status}">${result.status === 'ready' ? 'доступен' : result.status === 'empty' ? 'ждёт index.json' : 'нет связи'}</span></div>
    <div class="package-toolbar"><div class="surface-search">${icon('search')}<input id="packageSearch" placeholder="Поиск пакета или Java-библиотеки"></div><button class="secondary" id="addJar">+ Добавить .jar / Maven</button></div>
    <div class="package-grid" id="packageGrid">${cards}</div>
    <section class="trust-info"><h3>Как Funo проверяет пакет</h3><div><span>1</span> HTTPS с GitHub</div><div><span>2</span> SHA-256 совпадает</div><div><span>3</span> Версия записывается в funo.lock</div></section>
  </div>`);
  document.getElementById('refreshRegistry')!.onclick = () => void renderPackages();
  document.getElementById('packageSearch')!.oninput = e => {
    const q = (e.target as HTMLInputElement).value.toLowerCase();
    document.querySelectorAll<HTMLElement>('.package-card').forEach(c => c.classList.toggle('hidden', !c.dataset.search!.includes(q)));
  };
  document.querySelectorAll<HTMLElement>('.install-package').forEach(btn => btn.onclick = async () => {
    const pkg = result.packages.find(p => p.id === btn.dataset.id)!;
    try { const msg = await installPackage(project.root, pkg, false); toast(msg); btn.textContent = 'Установлено'; }
    catch (err) { toast(String(err), 'warn'); }
  });
  document.getElementById('addJar')!.onclick = () => openModal('Java-библиотека', `<p>Funo поддерживает обычные Java-библиотеки. Укажите Maven-координату:</p><label class="field">Maven ID<input placeholder="com.google.code.gson:gson:2.11.0"></label><label class="field">или локальный файл<input type="file" accept=".jar"></label><div class="modal-actions"><button class="secondary" data-close>Отмена</button><button class="primary" id="confirmJar">Добавить</button></div>`);
}

function packageCard(p: RegistryPackage) {
  return `<article class="package-card" data-search="${`${p.name} ${p.id} ${p.description}`.toLowerCase()}"><div class="package-icon">${p.kind === 'minecraft' ? icon('cube') : p.kind === 'java' ? icon('java') : icon('package')}</div><div class="package-main"><h3>${p.name}${p.verified ? `<span class="verified" title="SHA-256 указан">${icon('check')}</span>` : ''}</h3><code>${p.id}@${p.version}</code><p>${p.description}</p><footer><span>${p.kind}</span><button class="primary small install-package" data-id="${p.id}">Установить</button></footer></div></article>`;
}

function renderMinecraft() {
  showSurface(`<div class="surface-page minecraft-page"><div class="page-hero"><div><span class="overline">MINECRAFT + FUNO</span><h1>Новый мод без сложного Java-кода</h1><p>Funo создаст Gradle-проект, описание мода и простую точку запуска.</p></div><div class="voxel">F</div></div>
    <div class="wizard-grid"><section class="wizard"><h2>Создать проект</h2><label class="field">Название мода<input id="modName" value="Мой первый мод"></label><label class="field">ID мода<input id="modId" value="my_first_mod" pattern="[a-z0-9_]+"></label><label class="field">Загрузчик<div class="loader-options"><button class="loader active" data-loader="fabric"><b>Fabric</b><span>Проще начать</span></button><button class="loader" data-loader="forge"><b>Forge</b><span>Java-экосистема</span></button></div></label><button class="primary big" id="createMod">${icon('cube')} Создать Minecraft-мод</button></section>
    <section class="code-preview"><span>main.fun · Funo Minecraft API</span><pre><i>use</i> minecraft.<b id="loaderCode">fabric</b>\n\n<i>mod</i> <s>"my_first_mod"</s> {\n    <i>on</i> server_start {\n        <em>broadcast</em>(<s>"Сервер готов!"</s>)\n        <em>run_command</em>(<s>"time set day"</s>)\n    }\n    <i>on</i> player_join(player) {\n        <em>tell</em>(<s>"Добро пожаловать!"</s>)\n    }\n}</pre><div class="what-created"><b>Funo создаст:</b><span>✓ Gradle + manifest</span><span>✓ события Fabric / Forge</span><span>✓ Minecraft API-команды</span><span>✓ готовый JAR мода</span></div></section></div>
    <div class="learn-strip"><div>${icon('spark')}</div><p><b>Свои команды Funo без сложного Java-кода.</b><br>Доступны <code>log</code>, <code>broadcast</code>, <code>tell</code>, <code>give</code>, <code>actionbar</code> и <code>run_command</code>.</p></div>
  </div>`);
  let loader = 'fabric';
  document.querySelectorAll<HTMLElement>('.loader').forEach(x => x.onclick = () => { loader = x.dataset.loader!; document.querySelectorAll('.loader').forEach(y => y.classList.remove('active')); x.classList.add('active'); document.getElementById('loaderCode')!.textContent = loader; });
  const idInput = document.getElementById('modId') as HTMLInputElement;
  idInput.oninput = () => { document.querySelector('.code-preview s')!.textContent = `"${idInput.value}"`; };
  document.getElementById('createMod')!.onclick = async () => {
    const name = (document.getElementById('modName') as HTMLInputElement).value.trim();
    const modId = idInput.value.trim();
    if (!/^[a-z][a-z0-9_]{2,63}$/.test(modId)) { toast('ID: маленькие латинские буквы, цифры и _.', 'warn'); return; }
    try { project = await createMinecraftProject(name, modId, loader); models.forEach(m => m.dispose()); models.clear(); currentPath = project.files[0].path; updateProjectUI(); openFile(currentPath); toast('Minecraft-проект создан. Можно писать код!'); }
    catch (err) { toast(String(err), 'warn'); }
  };
}

const wiki = [
  ['Старт', 'Самый короткий код', `<h1>Самый короткий код</h1><p>Funo сам выводит типы и возвращаемые значения.</p><pre>fun hello(name: text) -> text = "Привет, " + name\n\nfun main() = println(hello("Мир"))</pre><div class="doc-note">Точки с запятой и <code>return(200)</code> необязательны.</div>`],
  ['Основы', 'Типы и переменные', `<h1>Типы и переменные</h1><p>Для обычных программ доступны числа, текст, логика, символы, массивы и коллекции.</p><pre>int score = 10\nlong worldSeed = 123456789L\nfloat speed = 1.5f\ndouble health = 19.75\ntext player = "Alex"\nbool online = true\nchar rank = 'A'\nint[] ids = [1, 2, 3]\nlist&lt;text&gt; names = ["Alex", "Steve"]\nmap&lt;text, int&gt; scores = map()\nscores.put("Alex", 42)</pre><p><code>let</code> создаёт неизменяемое значение, <code>var</code> — изменяемое, а тип можно написать через двоеточие: <code>let age: int = 18</code>.</p>`],
  ['Основы', 'Функции и циклы', `<h1>Функции и циклы</h1><pre>fun double(n: int) -> int = n * 2\n\nfun main() {\n    for i in 0..10 {\n        println(double(i))\n    }\n\n    int left = 3\n    while left &gt; 0 {\n        println(left)\n        left = left - 1\n    }\n}</pre><p>Также доступны <code>repeat</code>, <code>break</code>, <code>continue</code>, <code>and</code>, <code>or</code> и <code>not</code>.</p>`],
  ['CLI', 'Компилятор в терминале', `<h1>Funo в терминале</h1><p>CLI собирается вместе с проектом и сам устанавливается в пользовательский PATH.</p><pre>funo setup\nfuno check main.fun\nfuno run main.fun\nfuno build main.fun -o app.jar</pre><div class="doc-note">Для сборки нужен JDK 17 или 21. После <code>funo setup</code> откройте новый терминал.</div>`],
  ['Java', 'Java-библиотеки', `<h1>Java-библиотеки</h1><p>Установленные Java-пакеты автоматически попадают в classpath.</p><pre>use java "com.google.gson.Gson"\n\nfun main() {\n    var gson = new Gson()\n    println(gson.toJson("Привет"))\n}</pre>`],
  ['Minecraft', 'События и команды', `<h1>Minecraft API Funo</h1><p>Мастер создаёт настоящий Fabric/Forge Gradle-проект, мост событий и Funo API.</p><pre>use minecraft.fabric\n\nmod "hello_mod" {\n    on start {\n        log("Мод загружен")\n    }\n    on server_start {\n        broadcast("Сервер готов!")\n        run_command("time set day")\n    }\n    on player_join(player) {\n        tell("Добро пожаловать!")\n        give("minecraft:diamond", 1)\n    }\n}</pre>`],
  ['Пакеты', 'Официальный реестр', `<h1>Официальный реестр</h1><p>Пакеты загружаются только по HTTPS, сверяются по SHA-256 и записываются в <code>funo.lock</code>.</p><pre>funo pkg list\nfuno pkg search minecraft\nfuno pkg install funo.hello</pre><p>Источник: <code>github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL</code>. Для реестра нужен <code>index.json</code>; готовый пример находится в <code>registry-template</code>.</p>`]
];

function renderWiki() {
  showSurface(`<div class="surface-page wiki-page"><aside class="doc-nav"><div class="surface-search">${icon('search')}<input id="docSearch" placeholder="Поиск в вики"></div>${wiki.map((x, i) => `<button class="doc-link ${i === 0 ? 'active' : ''}" data-doc="${i}"><small>${x[0]}</small>${x[1]}</button>`).join('')}</aside><article class="doc-content" id="docContent">${wiki[0][2]}</article></div>`);
  document.querySelectorAll<HTMLElement>('.doc-link').forEach(x => x.onclick = () => { document.querySelectorAll('.doc-link').forEach(y => y.classList.remove('active')); x.classList.add('active'); document.getElementById('docContent')!.innerHTML = wiki[Number(x.dataset.doc)][2]; });
  (document.getElementById('docSearch') as HTMLInputElement).oninput = e => { const q = (e.target as HTMLInputElement).value.toLowerCase(); document.querySelectorAll<HTMLElement>('.doc-link').forEach((x, i) => x.classList.toggle('hidden', !wiki[i].join(' ').toLowerCase().includes(q))); };
}

function renderSettings() {
  showSurface(`<div class="surface-page settings-page"><div class="page-hero"><div><span class="overline">НАСТРОЙКИ</span><h1>Funo Studio</h1><p>Редактор хранит исходники локально. Код не отправляется в облако.</p></div></div><div class="settings-list"><section><h2>Редактор</h2><label><span><b>Режим по умолчанию</b><small>В новичке объяснения подробнее</small></span><select><option>Новичок</option><option>Профи</option></select></label><label><span><b>Автосохранение</b><small>После короткой паузы</small></span><input type="checkbox" checked></label></section><section><h2>Компилятор</h2><label><span><b>Java</b><small>Путь определяется автоматически</small></span><input value="javac"></label><label><span><b>Успешный код Funo</b><small>return(200) преобразуется в exit 0</small></span><input value="200"></label><div class="cli-card"><div>${icon('terminal')}</div><span><b>Funo CLI и PATH</b><small>Соберите CLI один раз, затем он установит себя в пользовательский PATH без прав администратора.</small><code>./scripts/install-cli.sh<br>PowerShell: .\\scripts\\install-cli.ps1</code></span><button class="secondary" id="copyCliSetup">Копировать</button></div></section><section><h2>Пакеты</h2><label><span><b>Официальный GitHub</b><small>Индекс проверенных библиотек</small></span><input value="vanyachickenganidanya-lgtm/funo_libsOFFICAL"></label><div class="cli-card compact"><div>${icon('package')}</div><span><b>Пакеты из консоли</b><small><code>funo pkg search &lt;имя&gt;</code> · <code>funo pkg install &lt;id&gt;</code></small></span></div></section></div></div>`);
  document.getElementById('copyCliSetup')!.onclick = async () => {
    await navigator.clipboard.writeText('./scripts/install-cli.sh\n# Windows PowerShell: .\\scripts\\install-cli.ps1');
    toast('Команды установки CLI скопированы.');
  };
}

function showSurface(html: string) {
  const surface = document.getElementById('surface')!; surface.innerHTML = html; surface.classList.remove('hidden');
}
function hideSurface() { const s = document.getElementById('surface')!; s.classList.add('hidden'); s.innerHTML = ''; }

function selectView(view: string) {
  currentView = view;
  document.querySelectorAll<HTMLElement>('.activity').forEach(x => x.classList.toggle('active', x.dataset.view === view));
  const title = document.getElementById('sidebarTitle')!;
  document.getElementById('newFile')!.classList.toggle('hidden', view !== 'explorer');
  if (view === 'explorer') { title.textContent = 'ПРОВОДНИК'; renderExplorer(); hideSurface(); }
  else if (view === 'search') { title.textContent = 'ПОИСК'; renderSearch(); hideSurface(); }
  else if (view === 'run') { title.textContent = 'ЗАПУСК И ОТЛАДКА'; renderRunSide(); hideSurface(); }
  else if (view === 'packages') { title.textContent = 'БИБЛИОТЕКИ'; document.getElementById('sidebarContent')!.innerHTML = `<div class="side-info">${icon('package')}<b>Funo Pack</b><p>Официальный реестр подключён к вашему GitHub.</p></div>`; void renderPackages(); }
  else if (view === 'minecraft') { title.textContent = 'MINECRAFT'; document.getElementById('sidebarContent')!.innerHTML = `<div class="side-info">${icon('cube')}<b>Создание модов</b><p>Fabric и Forge с простым кодом Funo.</p></div>`; renderMinecraft(); }
  else if (view === 'wiki') { title.textContent = 'ВИКИ FUNO'; document.getElementById('sidebarContent')!.innerHTML = `<div class="side-info">${icon('book')}<b>Документация</b><p>От первой функции до своего пакета.</p></div>`; renderWiki(); }
  else if (view === 'settings') { title.textContent = 'УПРАВЛЕНИЕ'; document.getElementById('sidebarContent')!.innerHTML = ''; renderSettings(); }
}

document.querySelectorAll<HTMLElement>('.activity').forEach(x => x.onclick = () => selectView(x.dataset.view!));

document.getElementById('newFile')!.onclick = async () => {
  const name = prompt('Название нового файла', 'feature.fun'); if (!name) return;
  const safe = name.endsWith('.fun') ? name : `${name}.fun`;
  if (project.files.some(f => f.path === safe)) return toast('Такой файл уже существует.', 'warn');
  const content = 'fun hello() {\n    println("Новая функция готова!")\n}';
  project.files.push({ path: safe, content }); await saveFile(project.root, safe, content); renderExplorer(); openFile(safe);
};

document.getElementById('refreshView')!.onclick = () => selectView(currentView);

editor.onDidChangeModelContent(() => {
  if (!editor.getModel()) return;
  const file = project?.files.find(f => f.path === currentPath); if (file) file.content = editor.getValue();
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

function escapeHtml(s: string) { return s.replace(/[&<>]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]!)); }

async function execute() {
  selectPanel('terminal');
  const panel = document.getElementById('panelBody')!;
  panel.innerHTML = '<span class="muted">› funo run\n  Проверяю типы и создаю Java…</span>';
  try {
    const minecraft = project.kind.startsWith('minecraft');
    const result = minecraft ? await buildMinecraft(project.root, editor.getValue()) : await runCode(project.root, editor.getValue());
    if (result.success) panel.innerHTML = `${escapeHtml(result.stdout)}${result.stdout ? '\n' : ''}<span class="success">✓ ${minecraft ? 'Minecraft-мод собран' : `Завершено успешно${/return\s*\(\s*200/.test(editor.getValue()) ? ' · Funo 200' : ''}`}</span>\n<span class="muted">${minecraft ? 'Gradle' : 'JVM'} · ${result.elapsed_ms} мс${result.artifact ? ` · ${escapeHtml(result.artifact)}` : ''}</span>`;
    else panel.innerHTML = `<span class="error">Сборка остановлена</span>\n${escapeHtml(result.stderr)}\n<span class="muted">Помощник покажет, как исправить код.</span>`;
    (window as any).__lastJava = result.generated_java;
    if (result.diagnostics.length) { diagnostics = result.diagnostics; renderFriendlyError(diagnostics[0]); }
  } catch (err) { panel.innerHTML = `<span class="error">Не удалось запустить</span>\n${escapeHtml(String(err))}`; }
}

document.getElementById('topRun')!.onclick = () => void execute();
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => void execute());
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => void saveFile(project.root, currentPath, editor.getValue()).then(() => toast('Файл сохранён.')));

document.getElementById('showJava')!.onclick = async () => {
  let java = (window as any).__lastJava;
  if (!java) { const result = await runCode(project.root, editor.getValue()); java = result.generated_java; (window as any).__lastJava = java; }
  openModal('Java, созданный компилятором Funo', `<p>Этот код передаётся в <code>javac</code>. Исходный .fun остаётся главным файлом.</p><pre class="java-preview">${escapeHtml(java || 'Java-код пока недоступен.')}</pre><div class="modal-actions"><button class="primary" data-close>Готово</button></div>`);
};

function selectPanel(panel: 'terminal' | 'problems' | 'output') { currentPanel = panel; document.querySelectorAll<HTMLElement>('.panel-tab').forEach(x => x.classList.toggle('active', x.dataset.panel === panel)); renderPanel(); }
document.querySelectorAll<HTMLElement>('.panel-tab').forEach(x => x.onclick = () => selectPanel(x.dataset.panel as any));
function renderPanel() {
  const body = document.getElementById('panelBody')!;
  if (currentPanel === 'problems') body.innerHTML = diagnostics.length ? diagnostics.map(d => `<button class="problem-line" data-line="${d.line}"><span class="${d.severity}">●</span> ${escapeHtml(d.title)} <em>[${d.code}] строка ${d.line}</em></button>`).join('') : '<span class="muted">Проблем не обнаружено.</span>';
  else if (currentPanel === 'output') body.innerHTML = '<span class="muted">Funo Language Server\nОфициальный реестр: vanyachickenganidanya-lgtm/funo_libsOFFICAL\nJVM backend: готов</span>';
  document.querySelectorAll<HTMLElement>('.problem-line').forEach(x => x.onclick = () => { editor.revealLineInCenter(Number(x.dataset.line)); editor.setPosition({ lineNumber: Number(x.dataset.line), column: 1 }); editor.focus(); });
}
document.getElementById('clearPanel')!.onclick = () => document.getElementById('panelBody')!.innerHTML = '';

function openModal(title: string, body: string) {
  const layer = document.getElementById('modalLayer')!;
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

document.getElementById('commandCenter')!.onclick = () => openModal('Команды Funo', `<div class="command-list"><button data-close onclick="document.getElementById('topRun').click()">${icon('play')} Запустить текущий файл <kbd>Ctrl Enter</kbd></button><button data-close>${icon('package')} Открыть библиотеки</button><button data-close>${icon('cube')} Создать Minecraft-мод</button><button data-close>${icon('book')} Открыть вики</button></div>`);

async function init() {
  try {
    project = await ensureProject(); updateProjectUI(); openFile(project.files.find(f => f.path === 'main.fun')?.path || project.files[0].path); setMode(mode);
  } catch (err) {
    document.getElementById('panelBody')!.innerHTML = `<span class="error">Не удалось открыть проект: ${escapeHtml(String(err))}</span>`;
  }
}
void init();
