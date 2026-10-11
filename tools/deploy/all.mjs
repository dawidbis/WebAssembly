// Pełne wdrożenie w bezpiecznej kolejności (każda strona toleruje starszą wersję drugiej):
//   1. infrastruktura + Lambda lobby (`infra.mjs`: plan, potwierdzenie, apply),
//   2. game-server (`game-server.mjs`),
//   3. frontend (`frontend.mjs --prep`),
//   4. sprawdzenie produkcji (`smoke.mjs`).
//
//   node tools/deploy/all.mjs [--yes] [--skip-infra] [--skip-server] [--skip-frontend]
//
// --yes  apply bez pytania (np. gdy plan był już przejrzany przez `infra.mjs --plan`)
//
// Przerywa na pierwszym błędzie. Restart game-servera kończy trwające gry (pokoje są w pamięci).
import { ROOT, run } from './aws.mjs';

const args = new Set(process.argv.slice(2));
const node = (script, extra = []) => run(process.execPath, [`tools/deploy/${script}`, ...extra], { cwd: ROOT });

if (!args.has('--skip-infra')) node('infra.mjs', args.has('--yes') ? ['--yes'] : []);
if (!args.has('--skip-server')) node('game-server.mjs');
if (!args.has('--skip-frontend')) node('frontend.mjs', ['--prep']);
node('smoke.mjs');
