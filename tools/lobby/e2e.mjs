// Test end-to-end lobby bez AWS: game-server w trybie biletów + Chromium (CDP), a odpowiedzi `/api/*`
// podstawia sam test przez przechwytywanie żądań w przeglądarce (CDP Fetch) – bez serwera-proxy.
// Bilety podpisuje CLI `ticket` (klucze z `ticket keygen`). Sprawdza: lobby widoczne, założenie pokoju
// → połączenie z biletem i mapa z seeda pokoju, druga karta w tym samym pokoju, ponowne połączenie
// z NOWYM biletem po restarcie serwera (bilet jest jednorazowy), brak błędów w konsoli.
//
//   cd web && npm run build && cd ..
//   cargo build -p game-server -p game-ticket
//   node tools/lobby/e2e.mjs [--shots katalog]      # Windows: CHROME=ścieżka do chrome.exe
import { execFileSync, spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { connect, launchChrome, sleep } from '../cdp.mjs';

const ROOT = resolve(fileURLToPath(import.meta.url), '../../..');
const EXE = process.platform === 'win32' ? '.exe' : '';
const GAME_SERVER = join(ROOT, `target/debug/game-server${EXE}`);
const TICKET = join(ROOT, `target/debug/ticket${EXE}`);
const PORT = 3077;
const PAGE = `http://127.0.0.1:${PORT}/`;
const argv = process.argv.slice(2);
const shots = argv.includes('--shots') ? argv[argv.indexOf('--shots') + 1] : null;

const keys = mkdtempSync(join(tmpdir(), 'lobby-e2e-'));
execFileSync(TICKET, ['keygen', '--out', keys]);
const privKey = join(keys, 'ticket.pem');
const pubKey = join(keys, 'ticket.pub.pem');

let gameServer;
const startServer = () => {
  gameServer = spawn(GAME_SERVER, ['--port', String(PORT), '--ticket-key', pubKey], { cwd: ROOT, stdio: 'ignore' });
};
startServer();
await sleep(1500);

// --- atrapa lobby (ten sam kształt odpowiedzi co crates/meta) ---
const rooms = [];
let ticketsIssued = 0;

/** Odpowiedź atrapy na `method path` z treścią `body` → [status, JSON]. */
function lobbyApi(method, path, body) {
  if (path === '/api/rooms' && method === 'GET') return [200, rooms];
  if (path === '/api/rooms' && method === 'POST') {
    const room = { id: `r${rooms.length + 1}`, name: String(body.name), seed: Number(body.seed ?? 5), players: 0, maxPlayers: 8, createdAt: 0 };
    rooms.push(room);
    return [201, room];
  }
  const id = /^\/api\/rooms\/(r\d+)\/join$/.exec(path)?.[1];
  const room = rooms.find((r) => r.id === id);
  if (room && method === 'POST') {
    ticketsIssued++;
    const args = ['--key', privKey, '--room', room.id, '--name', String(body.playerName), '--seed', String(room.seed)];
    const ticket = execFileSync(TICKET, args, { encoding: 'utf8' }).trim();
    return [200, { wsPath: `/ws?ticket=${ticket}`, room }];
  }
  return [404, { error: 'nie ma takiego zasobu' }];
}

// --- przeglądarka ---
const { chrome, port } = launchChrome('lobby-e2e');
const tabs = new Map();
let send;

/** Przechwycone żądanie `/api/*` – odpowiedź z atrapy. */
async function fulfill(params, sessionId) {
  const { request, requestId } = params;
  const body = request.postData ? JSON.parse(request.postData) : {};
  const [status, json] = lobbyApi(request.method, new URL(request.url).pathname, body);
  await send(
    'Fetch.fulfillRequest',
    {
      requestId,
      responseCode: status,
      responseHeaders: [{ name: 'content-type', value: 'application/json' }],
      body: Buffer.from(JSON.stringify(json)).toString('base64'),
    },
    sessionId,
  );
}

({ send } = await connect(port, ({ method, params, sessionId }) => {
  const tab = tabs.get(sessionId);
  if (!tab) return;
  switch (method) {
    case 'Fetch.requestPaused':
      void fulfill(params, sessionId);
      break;
    case 'Network.webSocketCreated':
      tab.wsUrls.push(params.url);
      break;
    case 'Network.webSocketFrameReceived':
      try {
        const msg = JSON.parse(params.response.payloadData);
        if (msg.type === 'welcome') tab.welcomes.push({ player: msg.player, seed: msg.config.map.seed });
      } catch {
        // ramka nie-JSON – pomijamy
      }
      break;
    case 'Runtime.exceptionThrown':
      tab.errors.push(params.exceptionDetails.text);
      break;
    case 'Runtime.consoleAPICalled':
      if (params.type === 'error') tab.errors.push(params.args.map((a) => a.value ?? a.description).join(' '));
      break;
  }
}));

async function openTab(name) {
  const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
  const tab = { name, sessionId, wsUrls: [], welcomes: [], errors: [] };
  tabs.set(sessionId, tab);
  await Promise.all(['Page', 'Runtime', 'Network'].map((domain) => send(`${domain}.enable`, {}, sessionId)));
  await send('Fetch.enable', { patterns: [{ urlPattern: '*/api/*', requestStage: 'Request' }] }, sessionId);
  await send('Page.navigate', { url: PAGE }, sessionId);
  return tab;
}
const evaluate = async (tab, expression) =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }, tab.sessionId)).result.value;
async function screenshot(tab, name) {
  if (!shots) return;
  mkdirSync(shots, { recursive: true });
  const { data } = await send('Page.captureScreenshot', { format: 'png' }, tab.sessionId);
  writeFileSync(join(shots, name), Buffer.from(data, 'base64'));
}

const results = [];
const check = (ok, what) => results.push({ ok, what });

const a = await openTab('A');
await sleep(4000);
check(await evaluate(a, `!!document.querySelector('app-lobby-panel .lobby')`), 'A: lobby widoczne (API dostępne)');
check(a.wsUrls.length === 0, 'A: przed wyborem pokoju brak połączenia z /ws');
await screenshot(a, 'lobby-lista.png');
await evaluate(
  a,
  `(() => { const f = document.querySelector('app-lobby-panel form'); f.room.value = 'Pokój testowy'; f.seed.value = '77'; f.requestSubmit(); return true; })()`,
);
await sleep(9000);
check(a.wsUrls.some((u) => u.includes('/ws?ticket=')), 'A: połączenie z biletem');
check(a.welcomes.at(-1)?.seed === 77 && a.welcomes.at(-1)?.player === 0, `A: Welcome z seedem 77 ${JSON.stringify(a.welcomes)}`);
check(
  await evaluate(a, `!!document.querySelector('app-lobby-panel .room-chip')?.textContent.includes('Pokój testowy')`),
  'A: lobby zwinięte do przycisku pokoju',
);
await screenshot(a, 'lobby-gra.png');

const b = await openTab('B');
await sleep(4000);
await evaluate(b, `(() => { document.querySelector('app-lobby-panel .rooms button').click(); return true; })()`);
await sleep(8000);
check(b.welcomes.at(-1)?.seed === 77 && b.welcomes.at(-1)?.player === 1, `B: w tym samym pokoju ${JSON.stringify(b.welcomes)}`);

// Restart serwera: klienci łączą się ponownie z nowym biletem.
const before = ticketsIssued;
gameServer.kill();
await sleep(1500);
startServer();
await sleep(16000);
check(ticketsIssued >= before + 2, `po restarcie serwera nowe bilety: ${ticketsIssued - before}`);
check(a.welcomes.length >= 2 && b.welcomes.length >= 2, `po restarcie Welcome w obu kartach (A ${a.welcomes.length}, B ${b.welcomes.length})`);
for (const tab of [a, b]) check(tab.errors.length === 0, `${tab.name}: brak błędów w konsoli ${tab.errors.join(' | ')}`);

for (const r of results) console.log(`${r.ok ? 'OK  ' : 'BŁĄD'} ${r.what}`);
gameServer.kill();
try {
  if (process.platform === 'win32') {
    const taskkill = join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'taskkill.exe');
    execFileSync(taskkill, ['/PID', String(chrome.pid), '/T', '/F'], { stdio: 'ignore' });
  } else {
    process.kill(-chrome.pid, 'SIGKILL');
  }
} catch {
  // przeglądarka już zamknięta
}
process.exit(results.every((r) => r.ok) ? 0 : 1);
