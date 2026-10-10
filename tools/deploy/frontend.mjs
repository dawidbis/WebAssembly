// Wdrożenie frontendu: build, upload do S3 z właściwym Cache-Control i Content-Type, inwalidacja.
//
//   node tools/deploy/frontend.mjs [--prep] [--skip-build]
//
// --prep        najpierw `npm run prep` (typy z Rusta + pakiet wasm) – po zmianach w Ruście
// --skip-build  wgraj istniejący web/dist bez budowania
// --dry-run     tylko pokaż podział plików (cache, Content-Type), nic nie wysyłaj
//
// Profil AWS: zmienna AWS_PROFILE (domyślnie `mapa`). Bucket i dystrybucja z `terraform output`.
//
// Cache: pliki z hashem w nazwie (main-XXXX.js, chunk-XXXX.js, styles-XXXX.css) są niezmienne – rok
// w cache. Pozostałe (index.html, wasm/game_wasm_bg.wasm, favicon) – `no-cache`, czyli rewalidacja
// ETagiem przy każdym wczytaniu (tania odpowiedź 304). index.html idzie na końcu, żeby nie wskazywał
// plików, których jeszcze nie ma. Stare pliki z hashem zostają w buckecie (stara karta może ich
// jeszcze potrzebować) – zajmują pojedyncze MB.
import { readdirSync, statSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import { ROOT, aws, run, tfOutputs } from './aws.mjs';

const args = process.argv.slice(2);
const WEB = resolve(ROOT, 'web');
const DIST = resolve(WEB, 'dist/web/browser');

// Hash esbuild: 8 znaków [A-Z0-9] (main-XXXX.js); chunki Pixi mają 8 znaków base64url (chunk-_-nztyU2.js).
const HASHED = /-[A-Za-z0-9_-]{8}\.(js|mjs|css)$/;
const IMMUTABLE = 'public,max-age=31536000,immutable';
const NO_CACHE = 'no-cache';
const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.ico': 'image/x-icon',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.txt': 'text/plain; charset=utf-8',
};

function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [relative(DIST, path).replaceAll('\\', '/')];
  });
}

if (args.includes('--prep')) run('npm', ['run', 'prep'], { cwd: WEB });
if (!args.includes('--skip-build')) run('npm', ['run', 'build'], { cwd: WEB });

const all = files(DIST);
const hashed = all.filter((f) => HASHED.test(f));
const rest = all.filter((f) => !HASHED.test(f) && f !== 'index.html');
const unknown = all.filter((f) => !TYPES[extname(f)]);
if (unknown.length) throw new Error(`brak Content-Type dla: ${unknown.join(', ')}`);

console.log(`\n${all.length} plików: ${hashed.length} z hashem, ${rest.length + 1} bez hasha\n`);
if (args.includes('--dry-run')) {
  for (const f of all) console.log(`${HASHED.test(f) ? 'immutable' : 'no-cache '}  ${TYPES[extname(f)].padEnd(28)} ${f}`);
  process.exit(0);
}

const { web_bucket: bucket, distribution_id: distribution, url } = tfOutputs();

// Jedno `s3 sync` na rozszerzenie: Content-Type podajemy jawnie (zgadywanie typu przez AWS CLI na
// Windows bierze go z rejestru – bywa `text/plain` dla .js, a przeglądarka odrzuca wtedy moduł).
function upload(list, cacheControl) {
  const byExt = Map.groupBy(list, (f) => extname(f));
  for (const [ext, group] of byExt) {
    aws([
      's3', 'sync', DIST, `s3://${bucket}`,
      '--exclude', '*', ...group.flatMap((f) => ['--include', f]),
      '--cache-control', cacheControl,
      '--content-type', TYPES[ext],
      '--only-show-errors',
    ]);
  }
}

upload(hashed, IMMUTABLE);
upload(rest, NO_CACHE);
upload(['index.html'], NO_CACHE);

aws(['cloudfront', 'create-invalidation', '--distribution-id', distribution, '--paths', '/', '/index.html', '/wasm/*']);
console.log(`\ngotowe: ${url}`);
