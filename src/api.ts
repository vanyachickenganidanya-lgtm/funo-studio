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
export type Project = { root: string; name: string; kind: string; files: ProjectFile[] };
export type MinecraftVersion = {
  id: string;
  label: string;
  stable: boolean;
  java: number;
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
    root: 'browser-preview', name: 'Мой первый проект', kind: 'console',
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

export async function createMinecraftProject(
  name: string,
  modId: string,
  loader: string,
  minecraftVersion: string
): Promise<Project> {
  if (isTauri()) return invoke<Project>('create_minecraft_project', { name, modId, loader, minecraftVersion });
  const safeName = name.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  return {
    root: 'browser-preview', name, kind: `minecraft-${loader}`,
    files: [
      { path: 'main.fun', content: `use minecraft.${loader}\n\nmod "${modId}" {\n    on start {\n        log("Мод ${safeName} загружен")\n    }\n\n    on server_start {\n        broadcast("Сервер Minecraft ${minecraftVersion} готов!")\n    }\n\n    on player_join(player) {\n        tell("Добро пожаловать!")\n    }\n}` },
      { path: 'funo.toml', content: `[project]\nname = "${safeName}"\nkind = "minecraft-${loader}"\ntarget = "jvm-${minecraftJava(minecraftVersion)}"\n\n[minecraft]\nmod_id = "${modId}"\nloader = "${loader}"\nversion = "${minecraftVersion}"\n` }
    ]
  };
}
