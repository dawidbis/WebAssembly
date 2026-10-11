// Test end-to-end lobby bez AWS: game-server w trybie biletów + Chromium (CDP), a odpowiedzi `/api/*`
// podstawia sam test przez przechwytywanie żądań w przeglądarce (CDP Fetch) – bez serwera-proxy.
// Bilety podpisuje CLI `ticket` (klucze z `ticket keygen`).
//
// Sprawdza: ekran powitalny bez generowania mapy, okno modalne „Utwórz lobby” z fokusem w formularzu
// i ustawieniami mapy, poczekalnię (skład na żywo, gospodarz, brak tur przed startem), zamknięcie lobby
// przez gospodarza (goście wracają do listy), zmianę ustawień zębatką (nowy `Welcome` u wszystkich),
// start tylko przez gospodarza, ponowne połączenie z NOWYM biletem po restarcie serwera (bilet jest
// jednorazowy), wyjście z gry do menu (gra kończy się, gdy wyjdą wszyscy) i brak błędów w konsoli.
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
const presence = new Set();
let ticketsIssued = 0;
let roomCounter = 0; // ID pokoi jak w Lambdzie: nigdy się nie powtarzają

/** Odpowiedź atrapy na `method path` z treścią `body` → [status, JSON]. */
function lobbyApi(method, path, body) {
  if (path === '/api/presence' && method === 'POST') {
    presence.add(String(body.clientId));
    return [200, { others: presence.size - 1 }];
  }
  if (path === '/api/presence/leave' && method === 'POST') {
    presence.delete(String(body.clientId));
    return [200, {}];
  }
  if (path === '/api/rooms' && method === 'GET') return [200, rooms];
  if (path === '/api/rooms' && method === 'POST') {
    const room = {
      id: `r${++roomCounter}`,
      name: String(body.name),
      seed: Number(body.seed ?? 5),
      mapWidth: { small: 1000, medium: 1400, large: 1800 }[body.map?.size ?? 'medium'],
      continents: Number(body.map?.continents ?? 3),
      settings: body.map ?? {},
      players: 0,
      maxPlayers: Number(body.maxPlayers ?? 8),
      createdAt: 0,
    };
    rooms.push(room);
    return [201, room];
  }
  const id = /^\/api\/rooms\/(r\d+)\/join$/.exec(path)?.[1];
  const room = rooms.find((r) => r.id === id);
  if (room && method === 'POST') {
    ticketsIssued++;
    const args = ['--key', privKey, '--room', room.id, '--name', String(body.playerName), '--client', String(body.clientId)];
    args.push('--room-name', room.name, '--max', String(room.maxPlayers), '--seed', String(room.seed), '--settings', JSON.stringify(room.settings));
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

function onFrame(tab, payload) {
  let msg;
  try {
    msg = JSON.parse(payload);
  } catch {
    return; // ramka nie-JSON
  }
  if (msg.type === 'welcome') tab.welcomes.push({ player: msg.player, seed: msg.config.map.seed, width: msg.config.map.width });
  if (msg.type === 'lobby') tab.lobby = msg;
  if (msg.type === 'turn') tab.turns++;
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
      onFrame(tab, params.response.payloadData);
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
  const tab = { name, sessionId, wsUrls: [], welcomes: [], lobby: null, turns: 0, errors: [] };
  tabs.set(sessionId, tab);
  await Promise.all(['Page', 'Runtime', 'Network'].map((domain) => send(`${domain}.enable`, {}, sessionId)));
  await send('Fetch.enable', { patterns: [{ urlPattern: '*/api/*', requestStage: 'Request' }] }, sessionId);
  await send('Page.navigate', { url: PAGE }, sessionId);
  return tab;
}
const evaluate = async (tab, expression) =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }, tab.sessionId)).result.value;
const $ = (selector) => `document.querySelector(${JSON.stringify('app-lobby-panel ' + selector)})`;
const setNick = (tab, nick) =>
  evaluate(tab, `(() => { const i = ${$('.nick input')}; i.value = ${JSON.stringify(nick)}; i.dispatchEvent(new Event('input')); return true; })()`);
const click = (tab, selector) => evaluate(tab, `(() => { ${$(selector)}?.click(); return true; })()`);
const roster = (tab) => tab.lobby?.players.map((p) => p.name) ?? [];
async function screenshot(tab, name) {
  if (!shots) return;
  mkdirSync(shots, { recursive: true });
  const { data } = await send('Page.captureScreenshot', { format: 'png' }, tab.sessionId);
  writeFileSync(join(shots, name), Buffer.from(data, 'base64'));
}

const results = [];
const check = (ok, what) => results.push({ ok, what });

// --- A: ekran powitalny, nic się nie generuje ---
const a = await openTab('A');
await sleep(4000);
check(await evaluate(a, `!!${$('.screen .rooms')}`), 'A: ekran powitalny z listą lobby');
check(await evaluate(a, `${$('.online strong')}?.textContent.trim() === '1'`), 'A: liczba osób na stronie = 1');
check(a.wsUrls.length === 0, 'A: przed wyborem lobby brak połączenia z /ws');
check((await evaluate(a, `performance.getEntriesByName('map-rendered').length`)) === 0, 'A: na ekranie powitalnym mapa się nie generuje');
await screenshot(a, '1-powitanie.png');

const nick = await evaluate(a, `${$('.nick input')}.value`);
check(/^guest\d{4}$/.test(nick), `A: domyślny nick guest + cyfry (${nick})`);

// --- A: okno modalne „Utwórz lobby” ---
await setNick(a, 'Ala');
await click(a, '.panel-head button.primary');
await sleep(300);
check(await evaluate(a, `!!${$('dialog.modal')}?.open`), 'A: okno modalne otwarte');
check(await evaluate(a, `document.activeElement?.name === 'room'`), 'A: fokus w polu nazwy lobby');
check(await evaluate(a, `${$('dialog form')}.room.value === 'Ala lobby'`), 'A: domyślna nazwa lobby „Ala lobby”');
await screenshot(a, '2-modal.png');
await evaluate(
  a,
  `(() => { const f = ${$('dialog form')}; f.room.value = 'Do zamknięcia'; f.seed.value = '5'; f.requestSubmit(); return true; })()`,
);
await sleep(4000);
check(a.wsUrls.some((u) => u.includes('/ws?ticket=')), 'A: połączenie z biletem');
check(await evaluate(a, `${$('.panel-head .leave')}?.textContent.trim() === 'Zamknij lobby'`), 'A (gospodarz): przycisk „Zamknij lobby”');

// --- B dołącza, gospodarz zamyka lobby: B wraca do listy z komunikatem ---
const b = await openTab('B');
await sleep(4000);
await setNick(b, 'Ola');
await click(b, '.rooms button');
await sleep(3000);
check(JSON.stringify(roster(b)) === '["Ala","Ola"]', `B w poczekalni: ${roster(b)}`);
check(await evaluate(b, `${$('.panel-head .leave')}?.textContent.trim() === 'Opuść lobby'`), 'B (gość): przycisk „Opuść lobby”');
await click(a, '.panel-head .leave');
await sleep(2000);
check(await evaluate(b, `!!${$('.rooms')} && ${$('.error')}?.textContent.includes('Gospodarz zamknął lobby')`), 'B: po zamknięciu lobby – lista i komunikat');
check(await evaluate(a, `!!${$('.rooms')}`), 'A: po zamknięciu lobby – lista');
await sleep(5500); // kolejne odświeżenie listy (co 5 s) – atrapa nadal zwraca pokój (bez heartbeatu)
const closedGone = `![...document.querySelectorAll('app-lobby-panel .room-name')].some((e) => e.textContent.includes('Do zamknięcia'))`;
const gone = await Promise.all([a, b].map((t) => evaluate(t, closedGone)));
[a, b].forEach((t, i) => check(gone[i], `${t.name}: zamknięte lobby nie wraca na listę`));
rooms.length = 0;

// --- nowe lobby z ustawieniami mapy ---
await click(a, '.panel-head button.primary');
await sleep(300);
await evaluate(
  a,
  `(() => { const f = ${$('dialog form')}; f.room.value = 'Pokój testowy'; f.seed.value = '77'; f.max.value = '4'; f.size.value = 'small'; f.continents.value = '2'; f.requestSubmit(); return true; })()`,
);
await sleep(4000);
check(a.welcomes.at(-1)?.seed === 77 && a.welcomes.at(-1)?.width === 1000, `A: Welcome z seedem 77 i małą mapą ${JSON.stringify(a.welcomes.at(-1))}`);
check(await evaluate(a, `${$('.panel-head .muted')}?.textContent.includes('mała · 2 kontynenty')`), 'A: opis mapy w poczekalni');
check(await evaluate(a, `!${$('dialog.modal')}.open && !!${$('.players')}`), 'A: modal zamknięty, widok poczekalni');
check(await evaluate(a, `!!${$('.panel-foot button.primary')}`), 'A (gospodarz): przycisk „Start gry”');

// --- B: dołącza do poczekalni ---
await sleep(5500); // lista B odświeża się co 5 s
await click(b, '.rooms button');
await sleep(4000);
check(JSON.stringify(roster(a)) === '["Ala","Ola"]' && JSON.stringify(roster(b)) === '["Ala","Ola"]', `skład na żywo w obu kartach: ${roster(a)} / ${roster(b)}`);
check(await evaluate(b, `!${$('.panel-foot button.primary')}`), 'B (gość): bez przycisku startu');
check(a.turns === 0 && b.turns === 0, 'w poczekalni nie lecą tury');
await screenshot(b, '3-poczekalnia.png');
await screenshot(a, '3a-poczekalnia-gospodarz.png');

// --- gospodarz zmienia ustawienia zębatką: nowa mapa u wszystkich ---
check(await evaluate(b, `!${$('.gear')}`), 'B (gość): bez zębatki ustawień');
await click(a, '.gear');
await sleep(300);
check(
  await evaluate(a, `(() => { const f = ${$('dialog form')}; return f.room.value === 'Pokój testowy' && f.seed.value === '77' && f.size.value === 'small'; })()`),
  'A: okno ustawień z bieżącymi ustawieniami',
);
await screenshot(a, '3b-ustawienia.png');
await evaluate(
  a,
  `(() => { const f = ${$('dialog form')}; f.room.value = 'Po zmianie'; f.seed.value = '99'; f.size.value = 'medium'; f.requestSubmit(); return true; })()`,
);
await sleep(3000);
const renamed = await Promise.all([a, b].map((t) => evaluate(t, `${$('#room-title')}?.textContent.trim() === 'Po zmianie'`)));
[a, b].forEach((t, i) => {
  const w = t.welcomes.at(-1);
  check(w?.seed === 99 && w?.width === 1400, `${t.name}: nowy Welcome po zmianie ustawień ${JSON.stringify(w)}`);
  check(renamed[i], `${t.name}: nowa nazwa lobby w poczekalni`);
});

// --- start przez gospodarza ---
await click(a, '.panel-foot button.primary');
await sleep(6000);
check(a.lobby?.started && b.lobby?.started, 'gra wystartowała u obu');
check(a.turns > 10 && b.turns > 10, `po starcie lecą tury (A ${a.turns}, B ${b.turns})`);
// Karta w tle nie odtwarza animacji – ekran może jeszcze czekać na koniec przejścia (klasa `screen-leave`).
const screenState = await evaluate(a, `${$('.screen')}?.className ?? 'brak'`);
check(
  (screenState === 'brak' || screenState.includes('screen-leave')) && (await evaluate(a, `!!${$('.exit-chip')}`)),
  `A: w grze ekran lobby znika (${screenState}), zostaje „Wyjdź do menu”`,
);
const rendered = await evaluate(a, `performance.getEntriesByName('map-rendered').length`);
check(rendered === 3, `A: mapa narysowana raz na konfigurację (zamknięte lobby, seed 77, seed 99): ${rendered}`);
await screenshot(a, '4-gra.png');

// --- restart serwera: ponowne połączenie z nowym biletem, pokój od nowa (poczekalnia) ---
const before = ticketsIssued;
const welcomesBefore = [a.welcomes.length, b.welcomes.length];
gameServer.kill();
await sleep(1500);
startServer();
await sleep(14000);
check(ticketsIssued >= before + 2, `po restarcie serwera nowe bilety: ${ticketsIssued - before}`);
check(
  a.welcomes.length > welcomesBefore[0] && b.welcomes.length > welcomesBefore[1],
  `po restarcie Welcome w obu kartach (A ${a.welcomes.length}, B ${b.welcomes.length})`,
);

// --- wyjście z gry do menu; gdy wyjdą wszyscy, gra się kończy ---
// Pokój po restarcie serwera jest od nowa w poczekalni – start daje ten, kto wrócił pierwszy (gospodarz).
await Promise.all([a, b].map((t) => click(t, '.panel-foot button.primary')));
await sleep(2000);
const health = async () => (await (await fetch(`http://127.0.0.1:${PORT}/health`)).json()).rooms;
check((await health()) === 1, 'gra trwa (1 pokój na serwerze)');
await click(b, '.exit-chip');
await sleep(300);
check(await evaluate(b, `!!document.querySelector('app-lobby-panel dialog.confirm')?.open`), 'B: potwierdzenie wyjścia z gry');
await screenshot(b, '5-wyjscie.png');
await click(b, 'dialog.confirm button.leave');
await sleep(1500);
check(await evaluate(b, `!!${$('.rooms')}`), 'B: po wyjściu z gry – menu główne (lista lobby)');
check((await health()) === 1, 'po wyjściu jednego gracza gra trwa');
await click(a, '.exit-chip');
await sleep(300);
await click(a, 'dialog.confirm button.leave');
await sleep(1500);
check((await health()) === 0, 'po wyjściu wszystkich graczy gra się zakończyła');
for (const tab of [a, b]) check(tab.errors.length === 0, `${tab.name}: brak błędów w konsoli ${tab.errors.join(' | ')}`);

for (const r of results) console.log(`${r.ok ? 'OK  ' : 'BŁĄD'} ${r.what}`);
gameServer.kill();
try {
  if (process.platform === 'win32') {
    const taskkill = join(process.env.SystemRoot ?? String.raw`C:\Windows`, 'System32', 'taskkill.exe');
    execFileSync(taskkill, ['/PID', String(chrome.pid), '/T', '/F'], { stdio: 'ignore' });
  } else {
    process.kill(-chrome.pid, 'SIGKILL');
  }
} catch {
  // przeglądarka już zamknięta
}
process.exit(results.every((r) => r.ok) ? 0 : 1);
