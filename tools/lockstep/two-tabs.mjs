// Test pętli lockstep na kilku kartach jednej przeglądarki (surowe CDP, bez zależności).
// Karta A wchodzi od razu, karta B po `--delay` s (nadrabia tury z `Welcome`). Skrypt podsłuchuje
// ramki WebSocket: hashe stanu odsyłane serwerowi, `Welcome`, tury i `Desync`.
//
//   node tools/lockstep/two-tabs.mjs [url] [--seconds 40] [--delay 8] [--tamper] [--shots katalog]
//
// --tamper  – trzecia karta psuje odsyłane hashe: serwer musi jej zgłosić desync (a A i B nie).
// --shots   – zrzuty ekranu kart na koniec testu.
// Chromium: zmienna CHROME (domyślnie /opt/pw-browsers/chromium).
import { mkdirSync, writeFileSync } from 'node:fs';

import { connect, launchChrome, sleep } from '../cdp.mjs';

const argv = process.argv.slice(2);
const option = (name, fallback) => {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : fallback;
};
const url = argv.find((a) => /^https?:\/\//.test(a)) ?? 'http://127.0.0.1:3000/';
const seconds = Number(option('--seconds', 40));
const delay = Number(option('--delay', 8));
const tamper = argv.includes('--tamper');
const shots = option('--shots', null);

const { chrome, port } = launchChrome('lockstep-profile');
const sessions = new Map(); // sessionId → karta
const { ws, send } = await connect(port, (m) => {
  if (m.sessionId && sessions.has(m.sessionId)) onEvent(sessions.get(m.sessionId), m.method, m.params);
});

function onEvent(tab, method, params) {
  switch (method) {
    case 'Network.webSocketFrameSent': {
      const msg = JSON.parse(params.response.payloadData);
      if (msg.type === 'hash') tab.hashes.set(msg.tick, msg.hash);
      break;
    }
    case 'Network.webSocketFrameReceived': {
      const msg = JSON.parse(params.response.payloadData);
      if (msg.type === 'welcome') tab.welcome = { player: msg.player, seed: msg.config.map.seed, catchup: msg.catchup.tick };
      else if (msg.type === 'turn') tab.lastTurn = msg.turn.tick;
      else if (msg.type === 'desync') tab.desyncs.push(msg.tick);
      break;
    }
    case 'Runtime.consoleAPICalled':
      if (params.type === 'error' || params.type === 'warning') {
        tab.console.push(`${params.type}: ${params.args.map((a) => a.value ?? a.description).join(' ')}`);
      }
      break;
    case 'Runtime.exceptionThrown':
      tab.console.push(`exception: ${params.exceptionDetails.exception?.description ?? params.exceptionDetails.text}`);
      break;
  }
}

/** Nowa karta; `evil` podmienia WebSocket.send tak, żeby odsyłane hashe były błędne. */
async function openTab(name, evil = false) {
  const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
  const tab = { name, sessionId, hashes: new Map(), desyncs: [], console: [], lastTurn: null, welcome: null, openedAt: Date.now() };
  sessions.set(sessionId, tab);
  await send('Page.enable', {}, sessionId);
  await send('Runtime.enable', {}, sessionId);
  await send('Network.enable', {}, sessionId);
  if (evil) {
    const source = `{
      const send = WebSocket.prototype.send;
      WebSocket.prototype.send = function (data) {
        const msg = JSON.parse(data);
        if (msg.type === 'hash') data = JSON.stringify({ ...msg, hash: (msg.hash ^ 1) >>> 0 });
        return send.call(this, data);
      };
    }`;
    await send('Page.addScriptToEvaluateOnNewDocument', { source }, sessionId);
  }
  await send('Page.navigate', { url }, sessionId);
  return tab;
}

const tabs = [await openTab('A')];
console.error(`A otwarta, B za ${delay} s, test ${seconds} s…`);
await sleep(delay * 1000);
tabs.push(await openTab('B'));
if (tamper) tabs.push(await openTab('C (psuje hashe)', true));
await sleep(Math.max(0, seconds - delay) * 1000);

if (shots) mkdirSync(shots, { recursive: true });
/** Liczba narysowanych map w karcie i (z `--shots`) zrzut ekranu. */
async function finishTab(tab) {
  const r = await send('Runtime.evaluate', { expression: `performance.getEntriesByName('map-rendered').length`, returnByValue: true }, tab.sessionId);
  tab.mapsRendered = r.result.value;
  if (shots) {
    const { data } = await send('Page.captureScreenshot', { format: 'png' }, tab.sessionId);
    writeFileSync(`${shots}/tab-${tab.name[0]}.png`, Buffer.from(data, 'base64'));
  }
}
await Promise.all(tabs.map(finishTab));
ws.close();
try { process.kill(-chrome.pid, 'SIGKILL'); } catch {}

// Porównanie hashy kart A i B (oraz C bez psucia – jej hashe są celowo błędne).
const honest = tabs.filter((t) => !t.name.startsWith('C'));
const ticks = [...honest[0].hashes.keys()].filter((tick) => honest.every((t) => t.hashes.has(tick)));
const mismatches = ticks.filter((tick) => new Set(honest.map((t) => t.hashes.get(tick))).size > 1);
const problems = [];
for (const tab of tabs) {
  const sent = [...tab.hashes.keys()];
  const lastHash = sent.length ? Math.max(...sent) : null;
  const lag = tab.lastTurn !== null && lastHash !== null ? tab.lastTurn - lastHash : null;
  console.log(
    `${tab.name}: gracz ${tab.welcome?.player ?? '?'}, seed ${tab.welcome?.seed ?? '?'}, nadrabiał ${tab.welcome?.catchup ?? '?'} tur, ` +
      `hashe ${sent.length} (ticki ${sent.length ? Math.min(...sent) : '–'}…${lastHash ?? '–'}), ostatnia tura ${tab.lastTurn ?? '–'}, ` +
      `opóźnienie gry ${lag ?? '–'} tur, desynce ${tab.desyncs.length}, map narysowanych ${tab.mapsRendered}`,
  );
  for (const line of tab.console) console.log(`  konsola: ${line}`);
  if (!tab.welcome) problems.push(`${tab.name}: brak Welcome`);
  if (sent.length < 5) problems.push(`${tab.name}: za mało hashy (${sent.length})`);
  if (lag === null || lag > 30) problems.push(`${tab.name}: gra nie nadąża za serwerem (${lag} tur)`);
  if (tab.mapsRendered !== 1) problems.push(`${tab.name}: map narysowanych ${tab.mapsRendered}, oczekiwana 1`);
  if (tab.console.some((l) => l.startsWith('error') || l.startsWith('exception'))) problems.push(`${tab.name}: błędy w konsoli`);
  if (tab.name.startsWith('C')) {
    if (!tab.desyncs.length) problems.push(`${tab.name}: serwer nie wykrył błędnych hashy`);
  } else if (tab.desyncs.length) {
    problems.push(`${tab.name}: desync na tickach ${tab.desyncs.slice(0, 5).join(', ')}`);
  }
}
if (honest.length > 1 && (tabs[1].welcome?.catchup ?? 0) === 0) problems.push('B nie dołączyła w trakcie gry (nadrabianie 0 tur)');
if (ticks.length < 5) problems.push(`za mało wspólnych ticków A i B (${ticks.length})`);
if (mismatches.length) problems.push(`różne hashe A i B na tickach ${mismatches.slice(0, 5).join(', ')}`);
console.log(`wspólne ticki A i B: ${ticks.length}, różne hashe: ${mismatches.length}`);
console.log(problems.length ? `BŁĄD:\n- ${problems.join('\n- ')}` : 'OK – bez desynców');
process.exit(problems.length ? 1 : 0);
