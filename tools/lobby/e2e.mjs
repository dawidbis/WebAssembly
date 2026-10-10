// Test end-to-end lobby bez AWS: atrapa `/api` (bilety podpisuje CLI `ticket`), game-server w trybie
// biletów i Chromium przez CDP. Sprawdza: lobby widoczne, założenie pokoju → połączenie z biletem
// i mapa z seeda pokoju, druga karta w tym samym pokoju, ponowne połączenie z NOWYM biletem po
// restarcie serwera (bilet jest jednorazowy), brak błędów w konsoli.
//
//   cd web && npm run build && cd ..
//   cargo build -p game-server -p game-ticket
//   node tools/lobby/e2e.mjs [--shots katalog]      # Windows: CHROME=ścieżka do chrome.exe
//
// Klucze Ed25519 generuje `openssl` (jest w Git for Windows) w katalogu tymczasowym.
import { execFileSync, spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { createServer, request } from 'node:http';
import { connect as netConnect } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { connect, launchChrome, sleep } from '../cdp.mjs';

const ROOT = resolve(fileURLToPath(import.meta.url), '../../..');
const EXE = process.platform === 'win32' ? '.exe' : '';
const GAME_SERVER = join(ROOT, `target/debug/game-server${EXE}`);
const TICKET = join(ROOT, `target/debug/ticket${EXE}`);
const GS_PORT = 3077;
const PORT = 3088;
const argv = process.argv.slice(2);
const shots = argv.includes('--shots') ? argv[argv.indexOf('--shots') + 1] : null;

const keys = mkdtempSync(join(tmpdir(), 'lobby-e2e-'));
const privKey = join(keys, 'ticket.pem');
const pubKey = join(keys, 'ticket.pub.pem');
execFileSync('openssl', ['genpkey', '-algorithm', 'ed25519', '-out', privKey]);
execFileSync('openssl', ['pkey', '-in', privKey, '-pubout', '-out', pubKey]);

let gs;
const startServer = () => {
  gs = spawn(GAME_SERVER, ['--port', String(GS_PORT), '--ticket-key', pubKey], { cwd: ROOT, stdio: 'ignore' });
};
startServer();

// --- atrapa lobby (ten sam kształt odpowiedzi co crates/meta) ---
const rooms = [];
let ticketsIssued = 0;
const json = (res, status, body) => {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify(body));
};
const readBody = (req) =>
  new Promise((done) => {
    let s = '';
    req.on('data', (d) => (s += d));
    req.on('end', () => done(s ? JSON.parse(s) : {}));
  });

async function api(req, res, url) {
  if (url.pathname === '/api/rooms' && req.method === 'GET') return json(res, 200, rooms);
  if (url.pathname === '/api/rooms' && req.method === 'POST') {
    const body = await readBody(req);
    const room = { id: `r${rooms.length + 1}`, name: body.name, seed: body.seed ?? 5, players: 0, maxPlayers: 8, createdAt: 0 };
    rooms.push(room);
    return json(res, 201, room);
  }
  const join = /^\/api\/rooms\/([^/]+)\/join$/.exec(url.pathname);
  const room = join && rooms.find((r) => r.id === join[1]);
  if (room && req.method === 'POST') {
    const { playerName } = await readBody(req);
    ticketsIssued++;
    const args = ['--key', privKey, '--room', room.id, '--name', playerName, '--seed', String(room.seed)];
    const ticket = execFileSync(TICKET, args, { encoding: 'utf8' }).trim();
    return json(res, 200, { wsPath: `/ws?ticket=${ticket}`, room });
  }
  return json(res, 404, { error: 'nie ma takiego zasobu' });
}

const server = createServer((req, res) => {
  const url = new URL(req.url, 'http://localhost');
  if (url.pathname.startsWith('/api/')) return void api(req, res, url);
  // Pozostałe ścieżki (pliki frontendu) – do game-servera.
  const upstream = request({ host: '127.0.0.1', port: GS_PORT, path: req.url, method: req.method, headers: req.headers }, (r) => {
    res.writeHead(r.statusCode, r.headers);
    r.pipe(res);
  });
  upstream.on('error', () => {
    res.writeHead(502);
    res.end();
  });
  req.pipe(upstream);
});
// WebSocket `/ws` – surowy tunel TCP do game-servera.
server.on('upgrade', (req, socket, head) => {
  const up = netConnect(GS_PORT, '127.0.0.1', () => {
    const headers = Object.entries(req.headers).map(([k, v]) => `${k}: ${v}`).join('\r\n');
    up.write(`${req.method} ${req.url} HTTP/1.1\r\n${headers}\r\n\r\n`);
    up.write(head);
    up.pipe(socket);
    socket.pipe(up);
  });
  up.on('error', () => socket.destroy());
  socket.on('error', () => up.destroy());
});
server.listen(PORT);
await sleep(1500);

// --- przeglądarka ---
const { chrome, port } = launchChrome('lobby-e2e');
const tabs = new Map();
const { send } = await connect(port, ({ method, params, sessionId }) => {
  const tab = tabs.get(sessionId);
  if (!tab) return;
  if (method === 'Network.webSocketCreated') tab.wsUrls.push(params.url);
  if (method === 'Network.webSocketFrameReceived') {
    try {
      const msg = JSON.parse(params.response.payloadData);
      if (msg.type === 'welcome') tab.welcomes.push({ player: msg.player, seed: msg.config.map.seed });
    } catch {
      // ramka nie-JSON – pomijamy
    }
  }
  if (method === 'Runtime.exceptionThrown') tab.errors.push(params.exceptionDetails.text);
  if (method === 'Runtime.consoleAPICalled' && params.type === 'error') {
    tab.errors.push(params.args.map((a) => a.value ?? a.description).join(' '));
  }
});

async function openTab(name) {
  const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
  const tab = { name, sessionId, wsUrls: [], welcomes: [], errors: [] };
  tabs.set(sessionId, tab);
  for (const domain of ['Page', 'Runtime', 'Network']) await send(`${domain}.enable`, {}, sessionId);
  await send('Page.navigate', { url: `http://127.0.0.1:${PORT}/` }, sessionId);
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
gs.kill();
await sleep(1500);
startServer();
await sleep(16000);
check(ticketsIssued >= before + 2, `po restarcie serwera nowe bilety: ${ticketsIssued - before}`);
check(a.welcomes.length >= 2 && b.welcomes.length >= 2, `po restarcie Welcome w obu kartach (A ${a.welcomes.length}, B ${b.welcomes.length})`);
for (const tab of [a, b]) check(tab.errors.length === 0, `${tab.name}: brak błędów w konsoli ${tab.errors.join(' | ')}`);

for (const r of results) console.log(`${r.ok ? 'OK  ' : 'BŁĄD'} ${r.what}`);
gs.kill();
server.close();
try {
  if (process.platform === 'win32') execFileSync('taskkill', ['/PID', String(chrome.pid), '/T', '/F'], { stdio: 'ignore' });
  else process.kill(-chrome.pid, 'SIGKILL');
} catch {
  // przeglądarka już zamknięta
}
process.exit(results.every((r) => r.ok) ? 0 : 1);
