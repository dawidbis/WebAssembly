// Symulacja wczytania gry: surowe CDP (spowolnienie CPU także dla workera), sieć dławiona serwerem.
// Użycie: node sim.mjs <url> <cpuRate>  → JSON z czasami
import { spawn } from 'node:child_process';
const [url, rateArg] = process.argv.slice(2);
const rate = Number(rateArg);
const port = 9300 + Math.floor(Math.random() * 500);
const chrome = spawn('/opt/pw-browsers/chromium', [
  '--headless=new', '--no-sandbox', `--remote-debugging-port=${port}`, '--no-first-run', '--no-default-browser-check',
  '--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--window-size=1280,800',
  `--user-data-dir=/tmp/sim-profile-${port}`, 'about:blank',
], { stdio: 'ignore', detached: true });
// Spowolnienie CPU całej przeglądarki (wszystkie wątki, także worker): jak cpulimit –
// w każdym okresie 20 ms proces działa 20/rate ms, a przez resztę jest wstrzymany.
let throttling = rate > 1;
(async () => {
  const period = 20;
  while (throttling) {
    try { process.kill(-chrome.pid, 'SIGCONT'); } catch {}
    await new Promise((r) => setTimeout(r, period / rate));
    if (!throttling) break;
    try { process.kill(-chrome.pid, 'SIGSTOP'); } catch {}
    await new Promise((r) => setTimeout(r, period - period / rate));
  }
  try { process.kill(-chrome.pid, 'SIGCONT'); } catch {}
})();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let version;
for (let i = 0; i < 150; i++) {
  try { version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json(); break; } catch { await sleep(200); }
}
const ws = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener('open', r));
let id = 0;
const pending = new Map();
const handlers = [];
ws.addEventListener('message', (ev) => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
  else handlers.forEach((h) => h(m));
});
const send = (method, params = {}, sessionId) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params, sessionId })); });
const throttled = [];
handlers.push(async (m) => {
  if (m.method === 'Target.attachedToTarget') {
    const sid = m.params.sessionId;
    const type = m.params.targetInfo.type;
    throttled.push(type);
    if (type === 'page') {
      await send('Network.enable', {}, sid);
      await send('Network.setCacheDisabled', { cacheDisabled: true }, sid);
      await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true }, sid);
    }
    await send('Runtime.runIfWaitingForDebugger', {}, sid);
  }
});
const { targetId } = await send('Target.createTarget', { url: 'about:blank' }).then((r) => r.result);
const { sessionId } = (await send('Target.attachToTarget', { targetId, flatten: true })).result;
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true }, sessionId);
await send('Runtime.enable', {}, sessionId);
await send('Page.enable', {}, sessionId);
await send('Page.navigate', { url }, sessionId);
const t0 = Date.now();
let result = null;
while (Date.now() - t0 < 900000) {
  const r = await send('Runtime.evaluate', {
    expression: `(() => { const m = performance.getEntriesByName('map-rendered')[0];
      const pr = performance.getEntriesByName('provinces-rendered')[0]; if (!m || !pr) return null;
      const nav = performance.getEntriesByType('navigation')[0];
      const res = performance.getEntriesByType('resource');
      const wasm = res.find((e) => e.name.endsWith('.wasm'));
      return JSON.stringify({ ready: Math.round(m.startTime), gen: Math.round(m.detail.generateMs),
        provinces: Math.round(pr.startTime), provincesMs: Math.round(pr.detail.provincesMs ?? 0),
        dom: Math.round(nav.domContentLoadedEventEnd), wasmEnd: wasm ? Math.round(wasm.responseEnd) : null,
        bytes: res.reduce((s, e) => s + e.transferSize, 0) + nav.transferSize }); })()`,
    returnByValue: true,
  }, sessionId);
  if (r.result?.result?.value) { result = JSON.parse(r.result.result.value); break; }
  await sleep(250);
}
console.log(JSON.stringify({ rate, ...result, wall: Date.now() - t0 }));
throttling = false;
ws.close();
try { process.kill(-chrome.pid, 'SIGCONT'); process.kill(-chrome.pid, 'SIGKILL'); } catch {}
process.exit(0);
