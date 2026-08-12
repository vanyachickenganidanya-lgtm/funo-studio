import { invoke } from '@tauri-apps/api/core';

export type Diagnostic = {
  severity: 'error' | 'warning' | 'info';
  line: number;
  column: number;
  end_column: number;
  code: string;
  title: string;
  message: string;
  example?: string;
  replacement?: string;
};

export type BuildResult = {
  success: boolean;
  stdout: string;
  stderr: string;
  generated_java: string;
  elapsed_ms: number;
  diagnostics: Diagnostic[];
  artifact?: string;
};

export type ProjectFile = { path: string; content: string };
export type Project = {
  root: string;
  name: string;
  kind: string;
  files: ProjectFile[];
  directories: string[];
  hidden_paths: string[];
};
export type MinecraftVersion = {
  id: string;
  label: string;
  stable: boolean;
  java: number;
};

export type MinecraftToolStatus = {
  found: boolean;
  compatible: boolean;
  managed: boolean;
  version: string;
  latest_version: string;
  path: string;
  detail: string;
  update_available: boolean;
};

export type StorageVolume = {
  id: string;
  root: string;
  install_root: string;
  free_bytes: number;
  total_bytes: number;
  available_after_bytes: number;
  eligible: boolean;
  current: boolean;
};

export type MinecraftToolchainStatus = {
  required_java: number;
  recommended_gradle: string;
  reserve_bytes: number;
  estimated_install_bytes: number;
  jdk: MinecraftToolStatus;
  gradle: MinecraftToolStatus;
  volumes: StorageVolume[];
  recommended_install_root: string;
  ready: boolean;
  has_updates: boolean;
  message: string;
};

export type RegistryPackage = {
  id: string;
  name: string;
  version: string;
  description: string;
  kind: 'funo' | 'java' | 'minecraft';
  source_url: string;
  sha256: string;
  verified: boolean;
  author?: string;
};

export type RegistryResponse = {
  source: string;
  status: 'ready' | 'empty' | 'offline';
  message: string;
  packages: RegistryPackage[];
};

const isTauri = () => '__TAURI_INTERNALS__' in window;

const demoCode = `fun fib(n: int) -> int = if n < 2 then n else fib(n - 1) + fib(n - 2)

fun main() {
    text title = "Привет из Funo Studio!"
    int answer = fib(10)
    bool ready = answer == 55

    println(title)
    println(answer)
    if ready {
        println("Типы и условия работают")
    }
    return(200)
}`;

function browserDiagnostics(source: string): Diagnostic[] {
  const typo = /\b(printn|pritnln|printl)\b/.exec(source);
  if (typo) {
    const before = source.slice(0, typo.index).split('\n');
    return [{
      severity: 'error', line: before.length, column: before.at(-1)!.length + 1,
      end_column: before.at(-1)!.length + 1 + typo[0].length, code: 'FUN001',
      title: `Похоже, в «${typo[0]}» опечатка`,
      message: 'Наверное, вы хотели вывести текст. Можно заменить это слово на println.',
      example: 'println("Привет!")', replacement: 'println'
    }];
  }
  const opens = [...source].filter(c => c === '{').length;
  const closes = [...source].filter(c => c === '}').length;
  if (opens > closes) return [{
    severity: 'error', line: source.split('\n').length, column: 1, end_column: 1,
    code: 'FUN002', title: 'Не хватает закрывающей скобки',
    message: 'Один блок начался с {, но пока не закончился. Добавить } в конец?',
    example: 'fun main() {\n    println("Готово")\n}', replacement: '\n}'
  }];
  return [];
}

export async function ensureProject(): Promise<Project> {
  if (isTauri()) return invoke<Project>('ensure_demo_project');
  const content = localStorage.getItem('funo-browser-code') || demoCode;
  return {
    root: 'browser-preview', name: 'Мой первый проект', kind: 'console', directories: ['src'], hidden_paths: [],
    files: [
      { path: 'main.fun', content },
      { path: 'funo.toml', content: '[project]\nname = "my-first-project"\ntarget = "jvm-21"\nsuccess_code = 200' },
      { path: 'src/minecraft.fun', content: 'use minecraft.fabric\n\nmod "hello_funo" {\n    on server_start {\n        broadcast("Сервер готов!")\n    }\n\n    on player_join(player) {\n        tell("Добро пожаловать!")\n    }\n}' }
    ]
  };
}

export async function saveFile(root: string, path: string, content: string): Promise<void> {
  if (isTauri()) return invoke('write_project_file', { projectRoot: root, relativePath: path, content });
  if (path === 'main.fun') localStorage.setItem('funo-browser-code', content);
}

export async function checkCode(source: string): Promise<Diagnostic[]> {
  if (isTauri()) return invoke<Diagnostic[]>('check_source', { source });
  return browserDiagnostics(source);
}

export async function runCode(root: string, source: string): Promise<BuildResult> {
  if (isTauri()) return invoke<BuildResult>('compile_and_run', { projectRoot: root, source, classpath: [] });
  const diagnostics = browserDiagnostics(source);
  if (diagnostics.length) return { success: false, stdout: '', stderr: diagnostics[0].message, generated_java: '', elapsed_ms: 0, diagnostics };
  const n = Number(/println\s*\(\s*fib\((\d+)\)\s*\)/.exec(source)?.[1] || 10);
  let a = 0, b = 1; for (let i = 0; i < n; i++) [a, b] = [b, a + b];
  const strings = [...source.matchAll(/println\s*\(\s*"([^"]*)"\s*\)/g)].map(m => m[1]);
  return {
    success: true, stdout: [...strings, String(a)].join('\n'), stderr: '', elapsed_ms: 23,
    generated_java: '// В desktop-версии Java генерирует Rust-компилятор Funo.', diagnostics: []
  };
}

export async function buildMinecraft(root: string, source: string): Promise<BuildResult> {
  if (isTauri()) return invoke<BuildResult>('build_minecraft', { projectRoot: root, source });
  const diagnostics = browserDiagnostics(source);
  return {
    success: diagnostics.length === 0,
    stdout: diagnostics.length ? '' : 'Browser preview: Java-мост создан. Desktop-версия запустит Gradle build.',
    stderr: diagnostics[0]?.message || '',
    generated_java: 'package funo.generated;\n\npublic final class FunoMain {\n    public static void start() { FunoMinecraft.log("Мод Funo загружен"); }\n    public static void serverStart(Object server) {\n        FunoMinecraft.bindServer(server);\n        FunoMinecraft.broadcast("Сервер готов!");\n    }\n    public static void playerJoin(Object player) {\n        FunoMinecraft.tell(player, "Добро пожаловать!");\n    }\n}',
    elapsed_ms: 18,
    diagnostics
  };
}

export async function fetchRegistry(): Promise<RegistryResponse> {
  const repo = 'https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL';
  if (isTauri()) return invoke<RegistryResponse>('fetch_registry', { repository: repo });
  return {
    source: repo, status: 'empty',
    message: 'Репозиторий подключён. Добавьте index.json по шаблону из проекта.', packages: []
  };
}

export async function installPackage(root: string, pkg: RegistryPackage, allowUnsafe: boolean): Promise<string> {
  if (isTauri()) return invoke<string>('install_package', { projectRoot: root, package: pkg, allowUnsafe });
  return `Предпросмотр: ${pkg.name} будет установлен в desktop-приложении.`;
}

const browserMinecraftVersions: Record<string, string[]> = {
  fabric: ['26.2', '26.1.2', '1.21.11', '1.21.1', '1.20.6', '1.20.1', '1.19.4', '1.18.2', '1.17.1', '1.16.5', '1.15.2', '1.14.4'],
  forge: ['26.2', '26.1.2', '1.21.11', '1.21.1', '1.20.6', '1.20.1', '1.19.4', '1.18.2', '1.17.1', '1.16.5', '1.15.2', '1.14.4'],
  neoforge: ['26.2', '26.1.2', '1.21.11', '1.21.10', '1.21.8', '1.21.6', '1.21.5', '1.21.4', '1.21.3', '1.21.1', '1.21', '1.20.6', '1.20.4', '1.20.3', '1.20.2']
};

function minecraftJava(version: string): number {
  const parts = version.split('.').map(Number);
  if (parts[0] >= 26) return 25;
  if (parts[1] > 20 || (parts[1] === 20 && (parts[2] || 0) >= 5)) return 21;
  if (parts[1] >= 18) return 17;
  if (parts[1] === 17) return 16;
  return 8;
}

export async function fetchMinecraftVersions(loader: string): Promise<MinecraftVersion[]> {
  if (isTauri()) return invoke<MinecraftVersion[]>('minecraft_versions', { loader });
  return (browserMinecraftVersions[loader] || []).map(id => ({ id, label: id, stable: true, java: minecraftJava(id) }));
}

export async function minecraftToolchainStatus(
  projectRoot: string,
  minecraftVersion: string,
  loader: string,
  checkUpdates = false
): Promise<MinecraftToolchainStatus> {
  if (isTauri()) return invoke<MinecraftToolchainStatus>('minecraft_toolchain_status', { projectRoot, minecraftVersion, loader, checkUpdates });
  const requiredJava = minecraftJava(minecraftVersion);
  const reserveBytes = 30 * 1024 ** 3;
  const jdkSize = 220 * 1024 ** 2;
  const gradleSize = 150 * 1024 ** 2;
  const recommendedGradle = requiredJava >= 25 ? '9.4.0' : requiredJava >= 21 ? '8.14.3' : '8.8';
  return {
    required_java: requiredJava, recommended_gradle: recommendedGradle, reserve_bytes: reserveBytes,
    estimated_install_bytes: jdkSize + gradleSize, recommended_install_root: '~/Funo Studio/tools',
    ready: false, has_updates: checkUpdates, message: 'Предпросмотр: в desktop-версии Studio проверит JDK и Gradle.',
    jdk: { found: false, compatible: false, managed: false, version: '', latest_version: String(requiredJava), path: '', detail: `Нужен JDK ${requiredJava}`, update_available: false },
    gradle: { found: false, compatible: false, managed: false, version: '', latest_version: recommendedGradle, path: '', detail: 'Нужен совместимый Gradle', update_available: false },
    volumes: [{ id: 'Системный диск', root: '/', install_root: '~/Funo Studio/tools', free_bytes: 80 * 1024 ** 3, total_bytes: 128 * 1024 ** 3, available_after_bytes: 80 * 1024 ** 3 - jdkSize - gradleSize, eligible: true, current: true }]
  };
}

export async function installMinecraftToolchain(
  projectRoot: string,
  minecraftVersion: string,
  loader: string,
  destinationRoot: string
): Promise<MinecraftToolchainStatus> {
  if (isTauri()) return invoke<MinecraftToolchainStatus>('install_minecraft_toolchain', { projectRoot, minecraftVersion, loader, destinationRoot });
  const status = await minecraftToolchainStatus(projectRoot, minecraftVersion, loader);
  status.ready = true;
  status.message = 'Предпросмотр: JDK и Gradle установлены.';
  status.jdk = { ...status.jdk, found: true, compatible: true, managed: true, version: String(status.required_java), path: destinationRoot, detail: `JDK ${status.required_java} готов` };
  status.gradle = { ...status.gradle, found: true, compatible: true, managed: true, version: status.recommended_gradle, path: destinationRoot, detail: `Gradle ${status.recommended_gradle} готов` };
  return status;
}

export async function createMinecraftProject(
  name: string,
  modId: string,
  loader: string,
  minecraftVersion: string
): Promise<Project> {
  if (isTauri()) return invoke<Project>('create_minecraft_project', { name, modId, loader, minecraftVersion });
  const safeName = name.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  return {
    root: 'browser-preview', name, kind: `minecraft-${loader}`, directories: [], hidden_paths: [],
    files: [
      { path: 'main.fun', content: `use minecraft.${loader}\n\nmod "${modId}" {\n    on start {\n        log("Мод ${safeName} загружен")\n    }\n\n    on server_start {\n        broadcast("Сервер Minecraft ${minecraftVersion} готов!")\n    }\n\n    on player_join(player) {\n        tell("Добро пожаловать!")\n    }\n}` },
      { path: 'funo.toml', content: `[project]\nname = "${safeName}"\nkind = "minecraft-${loader}"\ntarget = "jvm-${minecraftJava(minecraftVersion)}"\n\n[minecraft]\nmod_id = "${modId}"\nloader = "${loader}"\nversion = "${minecraftVersion}"\n` }
    ]
  };
}

export type StudioSettings = {
  onboarding_completed: boolean;
  beginner: boolean;
  installer_beginner_choice?: boolean;
  tutorial_step: number;
  compiler_backend: string;
  microsoft_client_id: string;
};

export type PathStatus = {
  installed: boolean;
  bin_dir: string;
  launcher: string;
  path_contains_bin: boolean;
};

export type InstalledMod = {
  project_id: string;
  version_id: string;
  name: string;
  file_name: string;
  sha512: string;
  source: string;
};

export type MinecraftInstance = {
  id: string;
  name: string;
  project_root: string;
  minecraft_version: string;
  loader: string;
  game_dir: string;
  jvm_args: string;
  game_args: string;
  launch_task: string;
  mods: InstalledMod[];
};

export type ModrinthProject = {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  icon_url?: string;
  downloads: number;
  versions: string[];
  categories: string[];
};

export type PluginProject = {
  id: string;
  name: string;
  language: string;
  kind: string;
  root: string;
  repository_hint: string;
};

export type PluginCheck = { success: boolean; summary: string; output: string };
export type MicrosoftAuthChallenge = {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
  message: string;
};
export type MinecraftAccount = { username: string; uuid: string; authenticated: boolean };

const defaultSettings: StudioSettings = {
  onboarding_completed: false,
  beginner: true,
  tutorial_step: 0,
  compiler_backend: 'jvm',
  microsoft_client_id: ''
};

export async function loadSettings(): Promise<StudioSettings> {
  if (isTauri()) return invoke<StudioSettings>('get_settings');
  try { return { ...defaultSettings, ...JSON.parse(localStorage.getItem('funo-settings') || '{}') }; }
  catch { return { ...defaultSettings }; }
}

export async function saveSettings(value: StudioSettings): Promise<StudioSettings> {
  if (isTauri()) return invoke<StudioSettings>('save_settings', { value });
  localStorage.setItem('funo-settings', JSON.stringify(value));
  return value;
}

export async function getPathStatus(): Promise<PathStatus> {
  if (isTauri()) return invoke<PathStatus>('path_status');
  return { installed: false, path_contains_bin: false, bin_dir: '~/.local/bin', launcher: '~/.local/bin/funo' };
}

export async function installPath(): Promise<PathStatus> {
  if (isTauri()) return invoke<PathStatus>('install_path');
  throw new Error('Установка PATH доступна в desktop-версии.');
}

export async function uninstallPath(): Promise<PathStatus> {
  if (isTauri()) return invoke<PathStatus>('uninstall_path');
  throw new Error('Управление PATH доступно в desktop-версии.');
}

export async function createFolder(root: string, path: string): Promise<Project> {
  if (isTauri()) return invoke<Project>('create_project_folder', { projectRoot: root, relativePath: path });
  const project = await ensureProject();
  if (!project.directories.includes(path)) project.directories.push(path);
  return project;
}

export async function reloadProject(root: string): Promise<Project> {
  if (isTauri()) return invoke<Project>('reload_project', { projectRoot: root });
  return ensureProject();
}

export async function setPathHidden(root: string, path: string, hidden: boolean): Promise<Project> {
  if (isTauri()) return invoke<Project>('set_project_path_hidden', { projectRoot: root, relativePath: path, hidden });
  const project = await ensureProject();
  project.hidden_paths = hidden ? [...new Set([...project.hidden_paths, path])] : project.hidden_paths.filter(value => value !== path);
  return project;
}

export async function runBackend(root: string, source: string, target: string, run: boolean): Promise<BuildResult> {
  if (target === 'jvm') return runCode(root, source);
  if (isTauri()) return invoke<BuildResult>('build_backend', { projectRoot: root, source, target, run });
  return {
    success: true,
    stdout: `Предпросмотр backend ${target}: desktop-версия создаст и ${run ? 'запустит' : 'соберёт'} программу.`,
    stderr: '', generated_java: `// ${target} preview generated from Funo`, elapsed_ms: 1, diagnostics: []
  };
}

export async function listInstances(): Promise<MinecraftInstance[]> {
  if (isTauri()) return invoke<MinecraftInstance[]>('list_instances');
  return JSON.parse(localStorage.getItem('funo-instances') || '[]');
}

export async function createInstance(name: string, root: string, version: string, loader: string): Promise<MinecraftInstance> {
  if (isTauri()) return invoke<MinecraftInstance>('create_instance', { name, projectRoot: root, minecraftVersion: version, loader });
  const instance: MinecraftInstance = { id: `preview-${Date.now()}`, name, project_root: root, minecraft_version: version, loader, game_dir: `preview/${name}`, jvm_args: '-Xmx2G', game_args: '', launch_task: 'runClient', mods: [] };
  const values = await listInstances(); values.push(instance); localStorage.setItem('funo-instances', JSON.stringify(values)); return instance;
}

export async function updateInstance(instance: MinecraftInstance): Promise<MinecraftInstance> {
  if (isTauri()) return invoke<MinecraftInstance>('update_instance', { instance });
  const values = (await listInstances()).map(value => value.id === instance.id ? instance : value); localStorage.setItem('funo-instances', JSON.stringify(values)); return instance;
}

export async function deleteInstance(id: string): Promise<void> {
  if (isTauri()) return invoke('delete_instance', { id });
  localStorage.setItem('funo-instances', JSON.stringify((await listInstances()).filter(value => value.id !== id)));
}

export async function launchInstance(id: string): Promise<string> {
  if (isTauri()) return invoke<string>('launch_instance', { id });
  return `Предпросмотр запуска ${id}. В desktop-версии будет выполнен изолированный runClient.`;
}

export async function searchModrinth(query: string, loader: string, version: string): Promise<ModrinthProject[]> {
  if (isTauri()) return invoke<ModrinthProject[]>('search_modrinth', { query, loader, gameVersion: version });
  return [];
}

export async function installModrinth(instanceId: string, projectId: string): Promise<MinecraftInstance> {
  if (isTauri()) return invoke<MinecraftInstance>('install_modrinth', { instanceId, projectId });
  throw new Error('Загрузка Modrinth доступна в desktop-версии.');
}

export async function removeInstanceMod(instanceId: string, projectId: string): Promise<MinecraftInstance> {
  if (isTauri()) return invoke<MinecraftInstance>('remove_instance_mod', { instanceId, projectId });
  throw new Error('Управление модами доступно в desktop-версии.');
}

export async function createPlugin(parent: string, name: string, language: string, kind = 'tooling'): Promise<PluginProject> {
  if (isTauri()) return invoke<PluginProject>('create_plugin', { parent, name, language, kind });
  return { id: name.toLowerCase().replace(/\W+/g, '-'), name, language, kind, root: `${parent}/${name}`, repository_hint: 'https://github.com/your-name/your-plugin' };
}

export async function checkPlugin(root: string): Promise<PluginCheck> {
  if (isTauri()) return invoke<PluginCheck>('check_plugin', { root });
  return { success: true, summary: 'Проверка доступна в desktop-версии', output: root };
}

export async function installPlugin(root: string): Promise<PluginProject> {
  if (isTauri()) return invoke<PluginProject>('install_plugin', { root });
  throw new Error('Установка плагина доступна в desktop-версии.');
}

export async function listPlugins(): Promise<PluginProject[]> {
  if (isTauri()) return invoke<PluginProject[]>('list_plugins');
  return [];
}

export async function beginMicrosoftAuth(): Promise<MicrosoftAuthChallenge> {
  if (isTauri()) return invoke<MicrosoftAuthChallenge>('begin_microsoft_auth');
  throw new Error('Microsoft-вход доступен в desktop-версии.');
}

export async function completeMicrosoftAuth(deviceCode: string): Promise<MinecraftAccount> {
  return invoke<MinecraftAccount>('complete_microsoft_auth', { deviceCode });
}

export async function currentMicrosoftAccount(): Promise<MinecraftAccount | null> {
  if (isTauri()) return invoke<MinecraftAccount | null>('current_microsoft_account');
  return null;
}

export async function logoutMicrosoft(): Promise<void> {
  if (isTauri()) return invoke('logout_microsoft');
}
