// 校验项目第三方许可清单和 MPL 源码是否逐字节进入最终 macOS 候选。
import { execFileSync } from 'node:child_process';
import { lstatSync, mkdtempSync, readFileSync, readdirSync, rmdirSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const tauriRoot = join(projectRoot, 'bifang_content_production_assistant_gui', 'src-tauri');
const entries = [
  'THIRD_PARTY_LICENSES.txt',
  'THIRD_PARTY_NOTICE.txt',
  'MPL-Sources/cssparser-0.36.0.crate',
  'MPL-Sources/cssparser-macros-0.6.1.crate',
  'MPL-Sources/dtoa-short-0.3.5.crate',
  'MPL-Sources/option-ext-0.2.0.crate',
  'MPL-Sources/selectors-0.36.1.crate',
];

function regularFile(path) {
  const info = lstatSync(path);
  if (!info.isFile() || info.isSymbolicLink()) throw new Error(`Not a regular file: ${path}`);
}

function verifyConfig() {
  const config = JSON.parse(readFileSync(join(tauriRoot, 'tauri.conf.json'), 'utf8'));
  const mappings = config.bundle?.macOS?.files;
  if (!mappings) throw new Error('macOS bundle file map is missing');
  for (const entry of entries) {
    const destination = `Resources/Legal/${entry}`;
    const source = join(projectRoot, 'legal', entry);
    if (mappings[destination] !== `../../legal/${entry}`) {
      throw new Error(`Unexpected Tauri legal mapping: ${destination}`);
    }
    if (resolve(tauriRoot, mappings[destination]) !== source) {
      throw new Error(`Tauri legal mapping escapes source: ${destination}`);
    }
    regularFile(source);
  }
}

function verifyApp(appPath) {
  const app = lstatSync(appPath);
  if (!app.isDirectory() || app.isSymbolicLink() || !appPath.endsWith('.app')) {
    throw new Error('Expected one regular .app directory');
  }
  verifyConfig();
  for (const entry of entries) {
    const source = join(projectRoot, 'legal', entry);
    const destination = join(appPath, 'Contents', 'Resources', 'Legal', entry);
    regularFile(destination);
    if (!readFileSync(source).equals(readFileSync(destination))) {
      throw new Error(`Bundled legal bytes differ: ${entry}`);
    }
  }
  const sourceNames = entries.filter((entry) => entry.startsWith('MPL-Sources/'))
    .map((entry) => entry.slice('MPL-Sources/'.length)).sort();
  const bundledNames = readdirSync(join(appPath, 'Contents', 'Resources', 'Legal', 'MPL-Sources')).sort();
  if (sourceNames.join('\n') !== bundledNames.join('\n')) {
    throw new Error('Bundled MPL source set differs');
  }
}

function verifyDmg(dmgPath) {
  regularFile(dmgPath);
  const root = mkdtempSync(join(tmpdir(), 'bifang-legal-dmg-'));
  const mount = join(root, 'volume');
  mkdirSync(mount);
  let device = null;
  let detached = false;
  try {
    const attached = execFileSync('/usr/bin/hdiutil', [
      'attach', '-readonly', '-nobrowse', '-noautoopen', '-mountpoint', mount, dmgPath,
    ], { encoding: 'utf8', timeout: 60000 });
    device = attached.match(/^\/dev\/\S+/m)?.[0];
    if (!device) throw new Error('Mounted DMG device is unknown');
    const apps = readdirSync(mount).filter((name) => name.endsWith('.app'));
    if (apps.length !== 1) throw new Error('Expected exactly one top-level app');
    verifyApp(join(mount, apps[0]));
    execFileSync('/usr/bin/hdiutil', ['detach', device], { timeout: 60000 });
    detached = true;
  } finally {
    if (device && !detached) {
      try {
        execFileSync('/usr/bin/hdiutil', ['detach', device], { timeout: 60000 });
        detached = true;
      } catch {
        // 不能在仍挂载时删除检查目录；将失败留给调用者处理。
      }
    }
    if (detached || !device) {
      rmdirSync(mount);
      rmdirSync(root);
    }
  }
  if (!detached) throw new Error('Read-only DMG detach failed');
}

const [mode, target] = process.argv.slice(2);
if (mode === '--config' && !target) {
  verifyConfig();
} else if (mode === '--app' && target) {
  verifyApp(resolve(target));
} else if (mode === '--dmg' && target) {
  verifyDmg(resolve(target));
} else {
  throw new Error('Usage: node scripts/verify-third-party-bundle.mjs --config | --app APP | --dmg DMG');
}
console.log('Third-party bundle verified');
