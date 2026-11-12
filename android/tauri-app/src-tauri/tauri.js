// Forwarder for the Gradle BuildTask (`node tauri android android-studio-script`).
// Hands off to the real @tauri-apps/cli entrypoint.
//
// The CLI location must be RESOLVED, never hardcoded. This file is committed, and
// it used to point at C:/Tools/QuotaCards/... on one developer's disk, so the
// Android build failed on every other machine, and would fail in CI, with
// "Cannot find module". That is one of the reasons no APK has shipped since
// v0.3.8: the toolchain was never the only problem.
import { createRequire } from 'module';
import { existsSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const require2 = createRequire(import.meta.url);
const here = path.dirname(fileURLToPath(import.meta.url));

const candidates = [
  // an explicit override always wins
  process.env.TAURI_CLI_JS,
  // the repo's own pinned copy, a sibling of tauri-app
  path.resolve(here, '../../tauri-cli-npm/node_modules/@tauri-apps/cli/tauri.js'),
  // a regular install at the repo root, which is what CI will do
  path.resolve(here, '../../../node_modules/@tauri-apps/cli/tauri.js'),
  // and whatever npm would resolve normally
  (() => {
    try {
      return require2.resolve('@tauri-apps/cli/tauri.js');
    } catch {
      return undefined;
    }
  })(),
].filter(Boolean);

const cli = candidates.find((p) => existsSync(p));
if (!cli) {
  console.error('tauri.js: cannot find the @tauri-apps/cli entrypoint. Tried:');
  for (const p of candidates) console.error(`  ${p}`);
  console.error('Install it (npm i @tauri-apps/cli) or set TAURI_CLI_JS.');
  process.exit(1);
}

process.argv[1] = cli;
require2(cli);
