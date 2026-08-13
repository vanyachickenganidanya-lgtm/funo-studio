import { access, readFile, writeFile } from "node:fs/promises";
import { constants as fsConstants } from "node:fs";
import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const isWindows = process.platform === "win32";
const tail = [];
const maxTailLines = 180;

function remember(chunk) {
  for (const line of String(chunk).replace(/\r/g, "").split("\n")) {
    if (!line) continue;
    tail.push(line);
    if (tail.length > maxTailLines) tail.shift();
  }
}

function executable(name) {
  return path.join(root, "node_modules", ".bin", `${name}${isWindows ? ".cmd" : ""}`);
}

async function exists(file) {
  try {
    await access(file, fsConstants.F_OK);
    return true;
  } catch {
    return false;
  }
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    console.log(`\n> ${command} ${args.join(" ")}`);
    const child = spawn(command, args, {
      cwd: root,
      env: process.env,
      shell: isWindows && /\.(?:bat|cmd)$/i.test(command),
      ...options,
    });
    child.stdout?.on("data", (chunk) => {
      process.stdout.write(chunk);
      remember(chunk);
    });
    child.stderr?.on("data", (chunk) => {
      process.stderr.write(chunk);
      remember(chunk);
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} завершился с кодом ${code ?? `signal ${signal}`}`));
    });
  });
}

async function ensureAmethyst() {
  const marker = path.join(root, "vendor", "amethyst", "app_pojavlauncher", "build.gradle");
  if (await exists(marker)) return;
  await run("git", ["submodule", "update", "--init", "--recursive", "--depth", "1"]);
  if (!(await exists(marker))) {
    throw new Error("Git submodule vendor/amethyst не был загружен");
  }
}

async function ensureAndroidSdk() {
  const sdkRoot = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT;
  if (!sdkRoot) return;
  const sdkManager = path.join(
    sdkRoot,
    "cmdline-tools",
    "latest",
    "bin",
    `sdkmanager${isWindows ? ".bat" : ""}`,
  );
  if (!(await exists(sdkManager))) return;
  await run(sdkManager, ["platforms;android-36", "build-tools;36.0.0", "ndk;27.3.13750724"]);
}

async function addJitPackRepository() {
  const candidates = [
    path.join(root, "src-tauri", "gen", "android", "settings.gradle.kts"),
    path.join(root, "src-tauri", "gen", "android", "settings.gradle"),
  ];
  const settings = (await Promise.all(candidates.map(async (file) => ((await exists(file)) ? file : null))))
    .find(Boolean);
  if (!settings) throw new Error("Tauri Android settings.gradle не найден; сначала выполните npm run android:init");

  const original = await readFile(settings, "utf8");
  if (/jitpack\.io/i.test(original)) return;
  const kotlin = settings.endsWith(".kts");
  const repository = kotlin
    ? 'maven(url = "https://jitpack.io")'
    : 'maven { url "https://jitpack.io" }';
  const updated = original.replace(/mavenCentral\(\)/g, `mavenCentral()\n        ${repository}`);
  if (updated === original) {
    throw new Error(`Не удалось добавить JitPack в ${path.relative(root, settings)}`);
  }
  await writeFile(settings, updated);
  console.log(`Добавлен JitPack в ${path.relative(root, settings)}`);
}

function emitGithubError(error) {
  if (!process.env.GITHUB_ACTIONS) return;
  const message = [...tail, String(error?.stack || error)]
    .join("\n")
    .slice(-48_000)
    .replace(/%/g, "%25")
    .replace(/\r/g, "%0D")
    .replace(/\n/g, "%0A");
  console.error(`::error title=Funo Android build failed::${message}`);
}

try {
  await ensureAmethyst();
  await ensureAndroidSdk();
  await addJitPackRepository();
  const args = ["android", "build", "--apk", "--ci"];
  if (process.argv.includes("--debug")) args.push("--debug");
  await run(executable("tauri"), args);
} catch (error) {
  emitGithubError(error);
  console.error(error);
  process.exitCode = 1;
}
