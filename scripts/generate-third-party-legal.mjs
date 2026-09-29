// 为 macOS 正式候选生成可复核的第三方许可证与 NOTICE；绝不访问线上服务。
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const guiRoot = join(projectRoot, 'bifang_content_production_assistant_gui');
const legalRoot = join(projectRoot, 'legal');
const mode = process.argv[2];
if (mode !== '--write' && mode !== '--check') {
  throw new Error('Usage: node scripts/generate-third-party-legal.mjs --write|--check');
}

const compare = (left, right) => (left < right ? -1 : left > right ? 1 : 0);
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const exactText = (file) => {
  const value = readFileSync(file, 'utf8');
  if (!value || value.includes('\uFFFD')) throw new Error(`Invalid legal text: ${file}`);
  return value;
};
const section = (heading, value) => `\n${'='.repeat(72)}\n${heading}\n${'='.repeat(72)}\n${value}${value.endsWith('\n') ? '' : '\n'}`;
const version = readFileSync(join(projectRoot, 'Cargo.toml'), 'utf8')
  .match(/^version = "([0-9]+\.[0-9]+\.[0-9]+)"$/m)?.[1];
if (!version) throw new Error('Workspace version is unavailable');
const guiVersion = JSON.parse(exactText(join(guiRoot, 'package.json'))).version;
if (guiVersion !== version) throw new Error('Cargo and GUI versions differ');

const rustReport = JSON.parse(execFileSync('cargo', [
  'about', 'generate', '--format', 'json', '--manifest-path',
  'bifang_content_production_assistant_gui/src-tauri/Cargo.toml',
  '--target', 'aarch64-apple-darwin', '--locked', '--offline', '--fail',
], { cwd: projectRoot, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }));
const nodeReport = JSON.parse(execFileSync('/bin/zsh', [
  '-lc', 'pnpm licenses list --prod --json',
], { cwd: guiRoot, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }));

const legalFileName = /^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)([-._ ].*|Notice\..*)?$/i;
const packageFiles = (root) => readdirSync(root)
  .filter((name) => legalFileName.test(name) && statSync(join(root, name)).isFile())
  .sort(compare)
  .map((name) => ({ name, text: exactText(join(root, name)) }));

const rustLicenses = new Map();
for (const license of rustReport.licenses) {
  if (!license.text) throw new Error(`Missing Rust license text: ${license.id}`);
  for (const item of license.used_by) {
    const key = `${item.crate.name}@${item.crate.version}`;
    const values = rustLicenses.get(key) ?? [];
    if (!values.some((value) => value.id === license.id && value.text === license.text)) {
      values.push({ id: license.id, text: license.text });
    }
    rustLicenses.set(key, values);
  }
}

const rust = rustReport.crates.map(({ package: item }) => {
  if (!item.source?.startsWith('registry+')) throw new Error(`Unexpected Rust source: ${item.name}`);
  const key = `${item.name}@${item.version}`;
  const files = packageFiles(dirname(item.manifest_path));
  const classified = rustLicenses.get(key) ?? [];
  if (!files.length && !classified.length) throw new Error(`No legal text for ${key}`);
  return { key, license: item.license, files, classified };
}).sort((left, right) => compare(left.key, right.key));
if (new Set(rust.map((item) => item.key)).size !== rust.length) throw new Error('Duplicate Rust crate');

const node = Object.values(nodeReport).flat().flatMap((item) => {
  if (item.versions.length !== item.paths.length) throw new Error(`Ambiguous npm paths: ${item.name}`);
  return item.versions.map((packageVersion, index) => {
    const key = `${item.name}@${packageVersion}`;
    const root = item.paths[index];
    const manifest = JSON.parse(exactText(join(root, 'package.json')));
    if (manifest.name !== item.name || manifest.version !== packageVersion) {
      throw new Error(`npm package identity drift: ${key}`);
    }
    const files = packageFiles(root);
    const missing = key === 'react-remove-scroll-bar@2.3.8';
    if (!files.length && (!missing || manifest.license !== 'MIT' || !/MIT/.test(exactText(join(root, 'README.md'))))) {
      throw new Error(`No packaged legal text: ${key}`);
    }
    return { key, license: item.license, files, author: manifest.author };
  });
}).sort((left, right) => compare(left.key, right.key));
if (new Set(node.map((item) => item.key)).size !== node.length) throw new Error('Duplicate npm package');

// npm 的该版本明确声明 MIT，但发布包没有 LICENSE 文件；保留此差异而不捏造版权年份。
const missingNpmNotice = [
  'react-remove-scroll-bar@2.3.8: the published npm package has no LICENSE file.',
  'Its package.json and README declare MIT; package.json names Anton Korzunov as author.',
  'The standard MIT permission/warranty terms below are supplied for reference,',
  'not represented as an original copyright notice from that version.',
].join('\n');
const standardMitTerms = [
  'MIT License',
  '',
  'Permission is hereby granted, free of charge, to any person obtaining a copy',
  'of this software and associated documentation files (the "Software"), to deal',
  'in the Software without restriction, including without limitation the rights',
  'to use, copy, modify, merge, publish, distribute, sublicense, and/or sell',
  'copies of the Software, and to permit persons to whom the Software is',
  'furnished to do so, subject to the following conditions:',
  '',
  'The above copyright notice and this permission notice shall be included in all',
  'copies or substantial portions of the Software.',
  '',
  'THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR',
  'IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,',
  'FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE',
  'AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER',
  'LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,',
  'OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE',
  'SOFTWARE.',
].join('\n');

let licenseText = [
  `Third-party license texts for Bifang Content Production Assistant ${version}`,
  'Target: macOS aarch64 GUI runtime; Rust from Cargo.lock and npm production packages from pnpm-lock.yaml.',
  'Original package license/notice files are reproduced by package below.',
  'For Rust packages without a top-level legal file, cargo-about 0.9.2 offline classified text is included.',
  'This file does not replace the application proprietary license or narrow third-party rights.',
  '',
].join('\n');
for (const item of rust) {
  licenseText += section(`Rust ${item.key} | declared: ${item.license}`, '');
  for (const file of item.files) licenseText += section(`Original ${file.name}`, file.text);
  if (!item.files.length) {
    for (const classified of item.classified.sort((a, b) => compare(a.id, b.id))) {
      licenseText += section(`cargo-about classified ${classified.id}`, classified.text);
    }
  }
}
for (const item of node) {
  licenseText += section(`npm ${item.key} | declared: ${item.license}`, '');
  for (const file of item.files) licenseText += section(`Original ${file.name}`, file.text);
  if (!item.files.length) licenseText += section('Declared MIT; original file absent', `${missingNpmNotice}\n\n${standardMitTerms}`);
}

const lock = exactText(join(projectRoot, 'Cargo.lock'));
const lockedChecksum = (name, packageVersion) => {
  const chunks = lock.split('[[package]]');
  const chunk = chunks.find((part) => part.includes(`\nname = "${name}"\n`) && part.includes(`\nversion = "${packageVersion}"\n`));
  const checksum = chunk?.match(/^checksum = "([a-f0-9]{64})"$/m)?.[1];
  if (!checksum) throw new Error(`Missing Cargo.lock checksum: ${name}@${packageVersion}`);
  return checksum;
};
const mpl = rust.filter((item) => item.license?.split(/[^A-Za-z0-9.-]+/).includes('MPL-2.0'));
const expectedMpl = [
  'cssparser-macros@0.6.1', 'cssparser@0.36.0', 'dtoa-short@0.3.5',
  'option-ext@0.2.0', 'selectors@0.36.1',
].sort(compare);
if (mpl.map((item) => item.key).sort(compare).join('\n') !== expectedMpl.join('\n')) {
  throw new Error('MPL source package set changed; review the new source obligation');
}
const mplSources = mpl.map((item) => {
  const [name, packageVersion] = item.key.split('@');
  const file = `${name}-${packageVersion}.crate`;
  const path = join(legalRoot, 'MPL-Sources', file);
  if (!existsSync(path) || !statSync(path).isFile()) throw new Error(`Missing MPL source archive: ${file}`);
  const expected = lockedChecksum(name, packageVersion);
  if (sha256(readFileSync(path)) !== expected) throw new Error(`MPL source checksum mismatch: ${file}`);
  return { key: item.key, file, checksum: expected };
});

let noticeText = [
  `Third-party NOTICE for Bifang Content Production Assistant ${version}`,
  'Target: macOS aarch64 GUI runtime.',
  `Included: ${rust.length} Rust runtime crates and ${node.length} npm production packages.`,
  'Exact original license and notice texts are in THIRD_PARTY_LICENSES.txt.',
  'The proprietary application license does not override third-party terms.',
  '',
  'Rust crates:',
  ...rust.map((item) => `- ${item.key} | ${item.license}`),
  '',
  'npm packages:',
  ...node.map((item) => `- ${item.key} | ${item.license}`),
  '',
  'MPL-2.0 covered source packages included in this DMG:',
  ...mplSources.map((item) => `- MPL-Sources/${item.file} | SHA-256 ${item.checksum}`),
  '',
  'The archive above is the exact source package identified by Cargo.lock for the listed version.',
  '',
  missingNpmNotice,
  '',
].join('\n');
for (const [key, fileName] of [
  ['aws-lc-sys@0.45.0', 'LICENSE'],
  ['tslib@2.8.1', 'CopyrightNotice.txt'],
]) {
  const item = [...rust, ...node].find((entry) => entry.key === key);
  const file = item?.files.find((entry) => entry.name === fileName);
  if (!file) throw new Error(`Missing required embedded notice: ${key}/${fileName}`);
  noticeText += section(`Original notice: ${key}/${fileName}`, file.text);
}

for (const [file, content] of [
  ['THIRD_PARTY_LICENSES.txt', licenseText],
  ['THIRD_PARTY_NOTICE.txt', noticeText],
]) {
  const destination = join(legalRoot, file);
  if (mode === '--check') {
    if (!existsSync(destination) || !readFileSync(destination).equals(Buffer.from(content))) {
      throw new Error(`Generated legal file drift: ${file}`);
    }
  } else {
    writeFileSync(destination, content);
  }
}
console.log(`Third-party legal ${mode === '--check' ? 'verified' : 'generated'}: ${rust.length} Rust, ${node.length} npm, ${mpl.length} MPL source archives`);
