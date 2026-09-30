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
http.createServer(async (req, res) => {
  let p = decodeURIComponent(req.url.split('?')[0]);
  let file = path.join(root, p === '/' ? 'index.html' : p);
  if (!fs.existsSync(file) || fs.statSync(file).isDirectory()) file = path.join(root, 'index.html');
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
  const CH = 16384;
  for (let o = 0; o < body.length; o += CH) {
    const part = body.subarray(o, o + CH);
    const now = Date.now();
    busyUntil = Math.max(busyUntil, now) + (part.length / rate) * 1000;
    await sleep(busyUntil - now);
    res.write(part);
  }
  res.end();
}).listen(Number(port));
