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

export const TARGET = 'aarch64-unknown-linux-musl';

/**
 * Kroskompilacja na Linux arm64 (`cargo zigbuild --release --target …`). Na Windows build bywa
 * przerywany blokadą pliku (os error 32 – np. antywirus skanuje duże archiwum `aws-sdk-dynamodb`):
 * wtedy druga próba po jednym zadaniu (`-j 1`), która dokańcza od miejsca przerwania.
 */
export function zigbuild(args) {
  const build = (extra) => run('cargo', ['zigbuild', '--release', ...extra, ...args, '--target', TARGET], { cwd: ROOT });
  try {
    build([]);
  } catch {
    console.warn('\nBuild przerwany – druga próba po jednym zadaniu (-j 1)…');
    try {
      build(['-j', '1']);
    } catch (e) {
      console.error(`\nBuild nie wyszedł. Potrzebne: rustup target add ${TARGET}; winget install zig.zig; cargo install cargo-zigbuild`);
      throw e;
    }
  }
}

export function aws(args) {
  run('aws', [...args, '--profile', PROFILE]);
}

/** Polecenie AWS CLI z wynikiem JSON. */
export function awsJson(args) {
  return JSON.parse(capture('aws', [...args, '--profile', PROFILE, '--output', 'json']));
}

/**
 * Skrypt powłoki na instancji game-servera przez SSM (`AWS-RunShellScript`); czeka na wynik.
 * Zwraca stdout; rzuca wyjątek, gdy skrypt się nie powiódł.
 */
export async function ssmRun(out, commands, comment) {
  const { Command } = awsJson([
    'ssm', 'send-command', '--region', out.region,
    '--instance-ids', out.game_server_instance_id,
    '--document-name', 'AWS-RunShellScript',
    '--comment', comment,
    '--parameters', JSON.stringify({ commands }),
  ]);
  console.log(`> SSM ${Command.CommandId} (${comment}) na ${out.game_server_instance_id}…`);
  let result;
  for (let i = 0; i < 60; i++) {
    await new Promise((r) => setTimeout(r, 2000));
    try {
      result = awsJson(['ssm', 'get-command-invocation', '--region', out.region, '--command-id', Command.CommandId, '--instance-id', out.game_server_instance_id]);
    } catch {
      continue; // wywołanie jeszcze niezarejestrowane
    }
    if (!['Pending', 'InProgress', 'Delayed'].includes(result.Status)) break;
  }
  if (result?.StandardErrorContent) console.error(result.StandardErrorContent);
  if (result?.Status !== 'Success') {
    console.log(result?.StandardOutputContent ?? '');
    throw new Error(`SSM (${comment}): ${result?.Status ?? 'brak odpowiedzi'}`);
  }
  return result.StandardOutputContent ?? '';
}

/** Wyjścia `terraform output -json` środowiska prod jako { nazwa: wartość }. */
export function tfOutputs() {
  const json = JSON.parse(capture('terraform', [`-chdir=${ENV_DIR}`, 'output', '-json']));
  return Object.fromEntries(Object.entries(json).map(([k, v]) => [k, v.value]));
}
