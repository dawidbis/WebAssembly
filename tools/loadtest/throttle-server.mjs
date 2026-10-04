// Serwer statyczny z symulacją sieci: opóźnienie (RTT) na żądanie, wspólny limit przepustowości,
// opcjonalny gzip. Użycie: node throttle-server.mjs <port> <katalog> <kbit/s|0> <rtt ms> <gzip|none>
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
const [port, root, kbps, rtt, mode] = process.argv.slice(2);
const rate = Number(kbps) * 1000 / 8; // bajty/s, 0 = bez limitu
const RTT = Number(rtt);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.ico': 'image/x-icon' };
const cache = new Map();
let busyUntil = 0; // wspólne łącze: kolejne bajty wysyłane po poprzednich
let connections = new WeakSet();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const base = path.resolve(root);
const index = path.join(base, 'index.html');
/** Plik z katalogu `root` dla ścieżki z adresu; spoza katalogu, brakujący albo katalog → index.html. */
function resolveFile(url) {
  let rel;
  try {
    rel = decodeURIComponent(url.split('?')[0]);
  } catch {
    return index;
  }
  const file = path.resolve(base, '.' + path.posix.normalize('/' + rel));
  if (!file.startsWith(base + path.sep)) return index;
  return fs.existsSync(file) && fs.statSync(file).isFile() ? file : index;
}
/** Wysyła kolejne kawałki odpowiedzi w tempie wspólnego łącza (bez `await` w pętli). */
async function pump(res, body, offset) {
  if (offset >= body.length) return res.end();
  const part = body.subarray(offset, offset + CHUNK);
  const now = Date.now();
  busyUntil = Math.max(busyUntil, now) + (part.length / rate) * 1000;
  await sleep(busyUntil - now);
  res.write(part);
  return pump(res, body, offset + CHUNK);
}
const CHUNK = 16384;
http.createServer(async (req, res) => {
  const file = resolveFile(req.url);
  // Nowe połączenie: TCP + TLS ≈ 2 RTT, każde żądanie: 1 RTT.
  const fresh = !connections.has(req.socket);
  connections.add(req.socket);
  await sleep(RTT * (fresh ? 3 : 1));
  const gz = mode === 'gzip' && /gzip/.test(req.headers['accept-encoding'] || '');
  const key = file + gz;
  if (!cache.has(key)) { const raw = fs.readFileSync(file); cache.set(key, gz ? zlib.gzipSync(raw, { level: 9 }) : raw); }
  const body = cache.get(key);
  const headers = { 'content-type': types[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-store' };
  if (gz) headers['content-encoding'] = 'gzip';
  res.writeHead(200, headers);
  if (!rate) return res.end(body);
  return pump(res, body, 0);
}).listen(Number(port));
