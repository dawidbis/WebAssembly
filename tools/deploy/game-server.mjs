// Wdrożenie game-servera: kroskompilacja na Linux arm64 (z cechą `aws` – heartbeat pokoi do lobby),
// binarka do S3, restart usługi przez SSM.
//
//   node tools/deploy/game-server.mjs [--skip-build]
//
// Wymaga: `rustup target add aarch64-unknown-linux-musl`, Zig i `cargo install cargo-zigbuild`
// (infra/README.md). Musl = statyczna binarka, niezależna od wersji glibc na instancji.
//
// Restart kończy trwające gry (pokoje są w pamięci procesu) – klienci łączą się ponownie sami.
// Poprzednie binarki zostają w S3 pod game-server/builds/<commit> (30 dni) – do ręcznego wycofania.
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT, aws, awsJson, capture, run, tfOutputs } from './aws.mjs';

const TARGET = 'aarch64-unknown-linux-musl';
const BINARY = resolve(ROOT, `target/${TARGET}/release/game-server`);
const args = new Set(process.argv.slice(2));

if (!args.has('--skip-build')) {
  try {
    run('cargo', ['zigbuild', '--release', '-p', 'game-server', '--features', 'aws', '--target', TARGET], { cwd: ROOT });
  } catch (e) {
    console.error(`\nBuild nie wyszedł. Potrzebne: rustup target add ${TARGET}; winget install zig.zig; cargo install cargo-zigbuild`);
    throw e;
  }
}
if (!existsSync(BINARY)) throw new Error(`brak binarki ${BINARY}`);

const out = tfOutputs();
const commit = capture('git', ['rev-parse', '--short', 'HEAD'], { cwd: ROOT }).trim();
const dirty = capture('git', ['status', '--porcelain'], { cwd: ROOT }).trim() ? '-dirty' : '';
const s3 = `s3://${out.artifacts_bucket}/${out.game_server_binary_key}`;

aws(['s3', 'cp', BINARY, `s3://${out.artifacts_bucket}/game-server/builds/${commit}${dirty}`, '--only-show-errors']);
aws(['s3', 'cp', BINARY, s3, '--only-show-errors']);

// Na instancji: podmiana binarki atomowo (mv), restart, sprawdzenie /health.
const script = [
  'set -euo pipefail',
  `aws s3 cp --region ${out.region} ${s3} /opt/game-server/game-server.new --only-show-errors`,
  'chmod 755 /opt/game-server/game-server.new',
  'mv -f /opt/game-server/game-server.new /opt/game-server/game-server',
  'systemctl restart game-server',
  'for i in $(seq 1 20); do curl -sf http://127.0.0.1:3000/health && exit 0; sleep 0.5; done',
  'echo "health nie odpowiada"; journalctl -u game-server -n 20 --no-pager; tail -n 20 /var/log/game-server/server.log; exit 1',
];
const { Command } = awsJson([
  'ssm', 'send-command', '--region', out.region,
  '--instance-ids', out.game_server_instance_id,
  '--document-name', 'AWS-RunShellScript',
  '--comment', `game-server ${commit}${dirty}`,
  '--parameters', JSON.stringify({ commands: script }),
]);
console.log(`> SSM ${Command.CommandId}: restart na ${out.game_server_instance_id}…`);

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
console.log(result?.StandardOutputContent ?? '');
if (result?.StandardErrorContent) console.error(result.StandardErrorContent);
if (result?.Status !== 'Success') throw new Error(`wdrożenie: ${result?.Status ?? 'brak odpowiedzi SSM'}`);
console.log(`\ngame-server ${commit}${dirty} działa – ${out.url}`);
