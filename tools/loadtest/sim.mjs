// Symulacja wczytania gry: surowe CDP (spowolnienie CPU także dla workera), sieć dławiona serwerem.
// Użycie: node sim.mjs <url> <cpuRate>  → JSON z czasami
import { connect, launchChrome, sleep } from '../cdp.mjs';

const [url, rateArg] = process.argv.slice(2);
const rate = Number(rateArg);
const { chrome, port } = launchChrome('sim-profile');

// Spowolnienie CPU całej przeglądarki (wszystkie wątki, także worker): jak cpulimit –
// w każdym okresie 20 ms proces działa 20/rate ms, a przez resztę jest wstrzymany.
const PERIOD = 20;
let throttling = rate > 1;
const signal = (sig) => {
  try {
    process.kill(-chrome.pid, sig);
  } catch {
    // przeglądarka już zamknięta
  }
};
/** Jeden krok cyklu: wznowienie (`running`) albo wstrzymanie, potem następny krok. */
function throttleStep(running) {
  if (!throttling) {
    signal('SIGCONT');
    return;
  }
  signal(running ? 'SIGCONT' : 'SIGSTOP');
  setTimeout(() => throttleStep(!running), running ? PERIOD / rate : PERIOD - PERIOD / rate);
}
if (throttling) throttleStep(true);

// Każdy nowy cel (strona, worker) czeka na debugger – włączamy w nim, co trzeba, i puszczamy.
const { ws, send } = await connect(port, (m) => {
  if (m.method === 'Target.attachedToTarget') void onAttached(m.params);
});
async function onAttached({ sessionId: sid, targetInfo }) {
  if (targetInfo.type === 'page') {
    await send('Network.enable', {}, sid);
    await send('Network.setCacheDisabled', { cacheDisabled: true }, sid);
    await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true }, sid);
  }
  await send('Runtime.runIfWaitingForDebugger', {}, sid);
}

const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true }, sessionId);
await send('Runtime.enable', {}, sessionId);
await send('Page.enable', {}, sessionId);
await send('Page.navigate', { url }, sessionId);
const t0 = Date.now();

const PROBE = `(() => { const m = performance.getEntriesByName('map-rendered')[0];
  const pr = performance.getEntriesByName('provinces-rendered')[0]; if (!m || !pr) return null;
  const nav = performance.getEntriesByType('navigation')[0];
  const res = performance.getEntriesByType('resource');
  const wasm = res.find((e) => e.name.endsWith('.wasm'));
  return JSON.stringify({ ready: Math.round(m.startTime), gen: Math.round(m.detail.generateMs),
    provinces: Math.round(pr.startTime), provincesMs: Math.round(pr.detail.provincesMs ?? 0),
    dom: Math.round(nav.domContentLoadedEventEnd), wasmEnd: wasm ? Math.round(wasm.responseEnd) : null,
    bytes: res.reduce((s, e) => s + e.transferSize, 0) + nav.transferSize }); })()`;

/** Czeka (co 250 ms, najwyżej 15 min) na znaczniki wczytania mapy i prowincji. */
async function waitForResult() {
  if (Date.now() - t0 >= 900000) return null;
  const r = await send('Runtime.evaluate', { expression: PROBE, returnByValue: true }, sessionId);
  if (r.result?.value) return JSON.parse(r.result.value);
  await sleep(250);
  return waitForResult();
}

const result = await waitForResult();
console.log(JSON.stringify({ rate, ...result, wall: Date.now() - t0 }));
throttling = false;
ws.close();
signal('SIGCONT');
signal('SIGKILL');
process.exit(0);
