import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

const repository = resolve(import.meta.dirname, '..');
const tauriRoot = join(repository, 'src-tauri');
const extension = process.platform === 'win32' ? '.exe' : '';

execFileSync('cargo', ['build', '--locked', '--release', '--bin', 'funo'], {
  cwd: tauriRoot,
  stdio: 'inherit'
});

const verboseVersion = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
const host = verboseVersion.match(/^host:\s*(.+)$/m)?.[1]?.trim();
if (!host) throw new Error('rustc did not report a host target triple');

const source = join(tauriRoot, 'target', 'release', `funo${extension}`);
const destinationDirectory = join(tauriRoot, 'binaries');
const destination = join(destinationDirectory, `funo-${host}${extension}`);
mkdirSync(destinationDirectory, { recursive: true });
copyFileSync(source, destination);
console.log(`Prepared Tauri console sidecar: ${destination}`);
