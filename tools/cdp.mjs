// Wspólne dla narzędzi testowych: Chromium bez okna i surowe CDP (Chrome DevTools Protocol)
// bez zależności. Używają go `loadtest/sim.mjs` i `lockstep/two-tabs.mjs`.
import { spawn } from 'node:child_process';
import { randomInt } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** `fn()` ponawiane do `tries` razy co `ms` – wynik pierwszej udanej próby albo ostatni błąd. */
export async function retry(fn, tries, ms) {
  try {
    return await fn();
  } catch (e) {
    if (tries <= 1) throw e;
    await sleep(ms);
    return retry(fn, tries - 1, ms);
  }
}

/** Chromium bez okna z portem debugowania (losowy z 9300–9799), we własnej grupie procesów. */
export function launchChrome(profile, extraArgs = []) {
  const port = randomInt(9300, 9800);
  const userDataDir = join(tmpdir(), profile + '-' + port);
  const chrome = spawn(process.env.CHROME ?? '/opt/pw-browsers/chromium', [
    '--headless=new', '--no-sandbox', `--remote-debugging-port=${port}`, '--no-first-run', '--no-default-browser-check',
    '--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--window-size=1280,800',
    `--user-data-dir=${userDataDir}`, ...extraArgs, 'about:blank',
  ], { stdio: 'ignore', detached: true });
  return { chrome, port };
}

/**
 * Adres WebSocket przeglądarki z `/json/version`, sprawdzony: tylko nasz lokalny port i ścieżka
 * DevTools z identyfikatorem w formacie GUID – odpowiedź nie może przekierować połączenia gdzie
 * indziej. Z odpowiedzi bierzemy wyłącznie GUID; schemat, host i port są nasze.
 */
function debuggerUrl(version, port) {
  const url = new URL(version.webSocketDebuggerUrl);
  const id = /^\/devtools\/browser\/([0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12})$/.exec(url.pathname)?.[1];
  if (url.protocol !== 'ws:' || url.hostname !== '127.0.0.1' || url.port !== String(port) || !id) {
    throw new Error(`nieoczekiwany adres DevTools: ${version.webSocketDebuggerUrl}`);
  }
  return `ws://127.0.0.1:${port}/devtools/browser/${id}`;
}

/**
 * Połączenie CDP z przeglądarką na `port` (czeka, aż wstanie). `send(method, params, sessionId)`
 * zwraca wynik albo odrzuca błąd CDP; `onEvent(m)` dostaje każde zdarzenie (wiadomość bez `id`).
 */
export async function connect(port, onEvent) {
  const version = await retry(async () => (await fetch(`http://127.0.0.1:${port}/json/version`)).json(), 150, 200);
  const ws = new WebSocket(debuggerUrl(version, port));
  await new Promise((r) => ws.addEventListener('open', r));
  let nextId = 0;
  const pending = new Map();
  ws.addEventListener('message', (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) {
      pending.get(m.id)(m);
      pending.delete(m.id);
    } else {
      onEvent(m);
    }
  });
  const send = (method, params, sessionId) =>
    new Promise((resolve, reject) => {
      const id = ++nextId;
      pending.set(id, (m) => (m.error ? reject(new Error(`${method}: ${m.error.message}`)) : resolve(m.result)));
      ws.send(JSON.stringify({ id, method, params: params ?? {}, sessionId }));
    });
  return { ws, send };
}
