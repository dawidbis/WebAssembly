// Sprawdzenie produkcji po wdrożeniu (tylko odczyt): strona, API lobby, WebSocket przez CloudFront
// i /health game-servera (przez SSM – /health nie jest wystawione na świat).
//
//   node tools/deploy/smoke.mjs [--skip-ssm]
//
// /health: wersja generatora musi być ta sama co w repo (`GENERATOR_VERSION`) – inaczej klienci
// z nowym frontendem dostaną inną mapę niż serwer; bilety muszą być włączone (lobby).
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { ROOT, ssmRun, tfOutputs } from './aws.mjs';

const args = new Set(process.argv.slice(2));
const out = tfOutputs();
const results = [];
const check = (ok, what) => {
  results.push(ok);
  console.log(`${ok ? 'OK  ' : 'BŁĄD'} ${what}`);
};

async function get(path) {
  try {
    const res = await fetch(`${out.url}${path}`, { redirect: 'manual' });
    return { status: res.status, type: res.headers.get('content-type') ?? '', text: await res.text() };
  } catch (e) {
    return { status: 0, type: '', text: String(e) };
  }
}

const page = await get('/');
check(page.status === 200 && page.type.includes('text/html'), `strona: ${page.status} ${page.type}`);

const rooms = await get('/api/rooms');
let list = null;
try {
  list = JSON.parse(rooms.text);
} catch {
  // nie JSON – niżej błąd
}
check(rooms.status === 200 && Array.isArray(list), `lobby /api/rooms: ${rooms.status}, ${Array.isArray(list) ? `${list.length} pokoi` : 'nie JSON'}`);

// Zwykłe GET bez nagłówków WebSocketu: odpowiada sam game-server (400/401/426); 5xx = origin nie działa.
const ws = await get('/ws');
check([400, 401, 426].includes(ws.status), `game-server przez CloudFront (/ws): ${ws.status}`);

if (!args.has('--skip-ssm')) {
  const source = readFileSync(resolve(ROOT, 'crates/mapgen/src/lib.rs'), 'utf8');
  const expected = Number(/pub const GENERATOR_VERSION: u32 = (\d+);/.exec(source)?.[1]);
  try {
    const health = JSON.parse(await ssmRun(out, ['curl -sf http://127.0.0.1:3000/health'], 'smoke /health'));
    check(health.status === 'ok', `game-server /health: ${health.status}, pokoje: ${health.rooms}`);
    check(health.generatorVersion === expected, `wersja generatora: serwer ${health.generatorVersion}, repo ${expected}`);
    check(health.tickets === true, `bilety (lobby) włączone: ${health.tickets}`);
  } catch (e) {
    check(false, `game-server /health przez SSM: ${e.message}`);
  }
}

console.log(results.every(Boolean) ? `\nProdukcja działa: ${out.url}` : '\nSą błędy – szczegóły wyżej.');
process.exit(results.every(Boolean) ? 0 : 1);
