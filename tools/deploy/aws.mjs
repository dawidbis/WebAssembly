// Wspólne pomocniki skryptów wdrożenia: uruchamianie poleceń i odczyt wyjść Terraform.
import { execFileSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export const ENV_DIR = resolve(ROOT, 'infra/envs/prod');
export const PROFILE = process.env.AWS_PROFILE || 'wieczko';

/** Uruchamia polecenie z wyjściem na konsolę; rzuca wyjątek przy kodzie ≠ 0. */
export function run(cmd, args, opts = {}) {
  console.log(`> ${cmd} ${args.join(' ')}`);
  // npm na Windows to plik .cmd – Node uruchamia go tylko przez powłokę.
  const shell = process.platform === 'win32' && cmd === 'npm';
  execFileSync(cmd, args, { stdio: 'inherit', shell, ...opts });
}

/** Uruchamia polecenie i zwraca stdout. */
export function capture(cmd, args, opts = {}) {
  return execFileSync(cmd, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], ...opts });
}

export function aws(args) {
  run('aws', [...args, '--profile', PROFILE]);
}

/** Wyjścia `terraform output -json` środowiska prod jako { nazwa: wartość }. */
export function tfOutputs() {
  const json = JSON.parse(capture('terraform', [`-chdir=${ENV_DIR}`, 'output', '-json']));
  return Object.fromEntries(Object.entries(json).map(([k, v]) => [k, v.value]));
}
