import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  copyFileSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const verifier = join(projectRoot, 'scripts', 'verify-third-party-bundle.mjs');
const entries = [
  'THIRD_PARTY_LICENSES.txt', 'THIRD_PARTY_NOTICE.txt',
  'MPL-Sources/cssparser-0.36.0.crate',
  'MPL-Sources/cssparser-macros-0.6.1.crate',
  'MPL-Sources/dtoa-short-0.3.5.crate',
  'MPL-Sources/option-ext-0.2.0.crate',
  'MPL-Sources/selectors-0.36.1.crate',
];

function run(...args) {
  return spawnSync(process.execPath, [verifier, ...args], { encoding: 'utf8' });
}

function withApp(check) {
  const root = mkdtempSync(join(tmpdir(), 'bifang-legal-test-'));
  const app = join(root, 'Candidate.app');
  try {
    for (const entry of entries) {
      const destination = join(app, 'Contents', 'Resources', 'Legal', entry);
      mkdirSync(dirname(destination), { recursive: true });
      copyFileSync(join(projectRoot, 'legal', entry), destination);
    }
    check(app);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('Tauri source mappings are exact', () => {
  assert.equal(run('--config').status, 0);
});

test('complete byte-identical legal bundle passes', () => {
  withApp((app) => assert.equal(run('--app', app).status, 0));
});

test('missing or changed legal bytes fail closed', () => {
  withApp((app) => {
    const legal = join(app, 'Contents', 'Resources', 'Legal');
    rmSync(join(legal, 'THIRD_PARTY_NOTICE.txt'));
    assert.notEqual(run('--app', app).status, 0);
    copyFileSync(join(projectRoot, 'legal', 'THIRD_PARTY_NOTICE.txt'), join(legal, 'THIRD_PARTY_NOTICE.txt'));
    writeFileSync(join(legal, 'THIRD_PARTY_NOTICE.txt'), 'changed');
    assert.notEqual(run('--app', app).status, 0);
  });
});

test('symlink and extra MPL source fail closed', () => {
  withApp((app) => {
    const legal = join(app, 'Contents', 'Resources', 'Legal');
    const notice = join(legal, 'THIRD_PARTY_NOTICE.txt');
    rmSync(notice);
    symlinkSync(join(projectRoot, 'legal', 'THIRD_PARTY_NOTICE.txt'), notice);
    assert.notEqual(run('--app', app).status, 0);
  });
  withApp((app) => {
    writeFileSync(join(app, 'Contents', 'Resources', 'Legal', 'MPL-Sources', 'extra.crate'), 'extra');
    assert.notEqual(run('--app', app).status, 0);
  });
});
