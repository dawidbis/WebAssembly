// Tabela szans zasobów: biom × ukształtowanie → rozkład na 27 zasobów (suma 100%).
// Wagi: wartości z tabeli GDD (zasoby dominujące) + tło z reguł (klimat dla zasobów biologicznych,
// geologia dla kopalin – kopaliny nie zależą od klimatu). Zero = wyjątek (zasób niemożliwy).
//
//   node tools/resources/chances.mjs            → crates/mapgen/src/resource_table.rs
//   node tools/resources/chances.mjs --md plik  → dodatkowo tabele w markdown (do dokumentu GDD)
//
// Kolejność zasobów (RES) = numery `Province.resource`; ta sama lista z kolorami jest w
// web/src/app/render/resources.ts – zmieniaj obie.
import { writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

const BIOMES = ['Las deszczowy', 'Sawanna', 'Pustynia', 'Step', 'Śródziemnomorski', 'Subtropikalny', 'Oceaniczny',
  'Gorące lato', 'Ciepłe lato', 'Borealny', 'Tajga', 'Tundra', 'Lądolód'];
const TYPES = ['Tropikalny', 'Tropikalny', 'Suchy', 'Suchy', 'Umiarkowany', 'Umiarkowany', 'Umiarkowany',
  'Kontynentalny', 'Kontynentalny', 'Kontynentalny', 'Polarny', 'Polarny', 'Polarny'];
const VARS = ['Płaski', 'Wyżyna', 'Góry', 'Rzeki i jeziora', 'Brzeg'];
const DRY = new Set([2, 3]);

// Kolejność wierszy w tabelach (grupy jak w legendzie: rolnictwo, hodowla, las, energia, metale, chemia).
const RES = ['Zboża', 'Ryż', 'Ziemniaki', 'Bawełna', 'Włókna roślinne', 'Tytoń', 'Trzcina cukrowa', 'Kawa', 'Herbata',
  'Kauczuk', 'Bydło', 'Konie', 'Wełna', 'Futra', 'Drewno', 'Węgiel kamienny', 'Węgiel brunatny',
  'Ropa naftowa', 'Uran', 'Rudy żelaza', 'Mangan', 'Chrom', 'Nikiel', 'Wolfram', 'Molibden', 'Miedź', 'Ołów', 'Cynk',
  'Aluminium', 'Cyna', 'Złoto', 'Surowce nawozowe', 'Cement', 'Saletra', 'Siarka'];

// Zasoby dominujące (tabela GDD, realia dwudziestolecia): [biom][wariant] = {zasób: waga}.
const U = `
0|Kauczuk 100, Trzcina cukrowa 80|Kawa 90, Aluminium 80, Kauczuk 60, Herbata 50, Rudy żelaza 40|Aluminium 100, Mangan 80, Rudy żelaza 70, Cyna 60|Ryż 90, Kauczuk 50, Cyna 40|Trzcina cukrowa 70, Kauczuk 40
1|Bydło 100, Bawełna 80, Konie 40|Rudy żelaza 60, Mangan 50, Kawa 50, Bydło 40|Mangan 90, Rudy żelaza 80, Chrom 70, Cyna 40|Ryż 80, Bydło 70, Bawełna 60, Trzcina cukrowa 50|Ryż 50, Bydło 40, Trzcina cukrowa 40
2|Ropa naftowa 100, Surowce nawozowe 90, Saletra 50|Ropa naftowa 80, Surowce nawozowe 70, Uran 50|Miedź 90, Chrom 80, Saletra 80, Uran 70, Siarka 60, Cyna 50|Surowce nawozowe 50, Ryż 30, Trzcina cukrowa 30|Ropa naftowa 40
3|Bydło 90, Konie 70, Bawełna 70|Wełna 70, Miedź 60, Bydło 50, Konie 50, Surowce nawozowe 40|Miedź 100, Saletra 90, Chrom 70, Siarka 50|Bydło 80, Zboża 70, Bawełna 60|Bydło 40
4|Tytoń 100, Bawełna 80, Zboża 60|Tytoń 70, Cement 60, Wełna 50, Rudy żelaza 50|Cement 90, Miedź 70, Ołów 60, Siarka 50|Ryż 90, Zboża 70, Trzcina cukrowa 60|Tytoń 60, Ryż 50
5|Zboża 100, Bydło 80, Tytoń 50|Herbata 70, Rudy żelaza 70, Cement 70, Węgiel kamienny 60, Wełna 40|Węgiel kamienny 90, Rudy żelaza 90, Ołów 80, Cynk 80|Zboża 90, Bydło 70, Drewno 60|Zboża 50, Bydło 40
6|Zboża 90, Drewno 60, Ziemniaki 60|Wełna 60, Cement 60, Rudy żelaza 50|Cement 90, Węgiel kamienny 80, Miedź 70, Cynk 70|Drewno 70, Zboża 70, Włókna roślinne 50|Drewno 40
7|Zboża 100, Bydło 90, Konie 50, Włókna roślinne 40|Rudy żelaza 70, Węgiel kamienny 60, Cement 60|Węgiel kamienny 90, Rudy żelaza 90, Mangan 80|Zboża 90, Bydło 80, Drewno 60|Zboża 50
8|Zboża 90, Ziemniaki 80, Drewno 60|Węgiel brunatny 70, Cement 70, Rudy żelaza 60|Węgiel kamienny 90, Mangan 90, Węgiel brunatny 80, Rudy żelaza 80|Drewno 70, Zboża 70, Włókna roślinne 60|Drewno 40
9|Drewno 60, Ziemniaki 40, Futra 15|Węgiel brunatny 80, Drewno 45|Węgiel brunatny 90, Rudy żelaza 80, Nikiel 80, Cement 70|Drewno 60, Futra 10|Drewno 40
10|Drewno 60, Futra 20|Węgiel brunatny 80, Futra 15|Nikiel 90, Wolfram 80, Chrom 70, Uran 60|Drewno 60, Futra 15|Drewno 35, Futra 10
11|Węgiel brunatny 70, Futra 25|Węgiel brunatny 90, Futra 15|Wolfram 100, Nikiel 100, Molibden 90, Uran 80|Węgiel brunatny 50, Futra 20|Futra 20`;
const listed = BIOMES.map(() => VARS.map(() => ({})));
for (const line of U.trim().split('\n')) {
  const [b, ...vs] = line.split('|');
  vs.forEach((v, i) => {
    for (const item of v.split(',')) {
      const m = item.trim().match(/^(.*) (\d+)$/);
      if (!RES.includes(m[1])) throw new Error(m[1]);
      listed[+b][i][m[1]] = +m[2];
    }
  });
}

// Przydatność klimatu dla zasobów biologicznych (0 = wyjątek), biomy w kolejności BIOMES.
const SUIT = {
  'Zboża': [0.2, 0.5, 0.1, 1, 0.8, 1, 1, 1, 1, 0.4, 0.15, 0, 0],
  'Ryż': [1, 0.7, 0.2, 0.3, 0.6, 0.8, 0.1, 0.4, 0.1, 0, 0, 0, 0],
  'Ziemniaki': [0, 0.1, 0, 0.3, 0.3, 0.4, 1, 0.5, 1, 0.8, 0.5, 0.1, 0],
  'Bawełna': [0.3, 1, 0.3, 0.8, 1, 0.8, 0, 0.5, 0, 0, 0, 0, 0],
  'Włókna roślinne': [0.2, 0.3, 0.1, 0.5, 0.5, 0.6, 0.8, 0.7, 1, 0.6, 0.3, 0, 0],
  'Tytoń': [0.6, 0.6, 0.1, 0.3, 1, 1, 0.1, 0.7, 0.2, 0, 0, 0, 0],
  'Trzcina cukrowa': [1, 0.8, 0.2, 0, 0.3, 0.6, 0, 0.1, 0, 0, 0, 0, 0],
  'Kawa': [1, 0.6, 0, 0, 0.1, 0.3, 0, 0, 0, 0, 0, 0, 0],
  'Herbata': [0.7, 0.2, 0, 0, 0.2, 1, 0.2, 0.1, 0, 0, 0, 0, 0],
  'Kauczuk': [1, 0.3, 0, 0, 0, 0.1, 0, 0, 0, 0, 0, 0, 0],
  'Bydło': [0.2, 1, 0.2, 1, 0.6, 1, 1, 1, 0.9, 0.4, 0.1, 0, 0],
  'Konie': [0, 0.6, 0.2, 1, 0.5, 0.6, 0.6, 1, 0.8, 0.4, 0.2, 0, 0],
  'Wełna': [0, 0.3, 0.3, 1, 0.8, 0.6, 0.8, 0.7, 0.6, 0.3, 0.2, 0.1, 0],
  'Futra': [0, 0, 0, 0.3, 0, 0.1, 0.4, 0.3, 0.6, 0.6, 0.6, 0.6, 0.5],
  'Drewno': [1, 0.3, 0.1, 0.2, 0.5, 0.8, 1, 0.6, 1, 1, 1, 0.05, 0],
};
// Ukształtowanie dla grup biologicznych: [Płaski, Wyżyna, Góry, Rzeki i jeziora, Brzeg].
const CROP = [1, 0.5, 0.15, 1.2, 0.8];
const HIGH_CROP = [0.4, 1, 0.3, 0.5, 0.4]; // kawa i herbata – plantacje na wyżynach
const HERD = [1, 0.8, 0.4, 1, 0.7];
const SHEEP = [0.8, 1, 0.6, 0.7, 0.7];
const WOOD = [1, 0.9, 0.5, 1, 0.8];
const FUR = [1, 1, 0.6, 1, 0.8];
const BIO_TERRAIN = {
  'Zboża': CROP, 'Ryż': CROP, 'Ziemniaki': CROP, 'Bawełna': CROP, 'Włókna roślinne': CROP, 'Tytoń': CROP,
  'Trzcina cukrowa': CROP, 'Kawa': HIGH_CROP, 'Herbata': HIGH_CROP, 'Kauczuk': CROP,
  'Bydło': HERD, 'Konie': HERD, 'Wełna': SHEEP, 'Futra': FUR, 'Drewno': WOOD,
};
const BIO_BASE = 8;

// Kopaliny: waga bazowa i ukształtowanie (geologia, bez zależności od klimatu).
const ORE = [0.25, 0.6, 1, 0.15, 0.15];
const MINERAL = {
  'Rudy żelaza': [6, ORE], 'Węgiel kamienny': [5, ORE], 'Węgiel brunatny': [5, [0.6, 1, 0.6, 0.4, 0.3]],
  'Miedź': [4, ORE], 'Mangan': [3, ORE], 'Chrom': [3, ORE], 'Ołów': [3, ORE], 'Cynk': [3, ORE],
  'Nikiel': [2, ORE], 'Aluminium': [2, ORE], 'Cyna': [2, [0.25, 0.6, 1, 0.5, 0.15]], 'Uran': [2, ORE],
  'Wolfram': [1.5, ORE], 'Molibden': [1, ORE],
  // Złoto: bardzo rzadkie, tylko złoża kruszcowe w górach i złoto okruchowe w rzekach (zwykle 1–2 prowincje na mapę,
  // generator gwarantuje co najmniej jedną – `resources.rs`).
  'Złoto': [1, [0, 0, 1, 0.4, 0]],
  'Ropa naftowa': [4, [1, 0.5, 0.1, 0.5, 1]],
  'Surowce nawozowe': [3, [1, 1, 0.5, 0.6, 0.6]],
  'Cement': [4, [0.5, 1, 1, 0.5, 0.5]],
  'Saletra': [2, [0.6, 0.6, 1, 0.3, 0.3]],
  'Siarka': [2, [0.3, 0.6, 1, 0.3, 0.3]],
};
const TROPICAL = new Set([0, 1]);

function background(r, b, v) {
  if (SUIT[r]) return BIO_BASE * SUIT[r][b] * BIO_TERRAIN[r][v];
  const [base, terr] = MINERAL[r];
  let w = base * terr[v];
  // Boksyt (ruda aluminium) to zwietrzelina laterytowa, cyna – złoża okruchowe: głównie tropiki.
  if (r === 'Aluminium' || r === 'Cyna') w *= TROPICAL.has(b) ? 2 : 0.5;
  if ((r === 'Surowce nawozowe' || r === 'Saletra' || r === 'Siarka') && DRY.has(b)) w *= 1.5;
  return w;
}

/** Procenty z dokładnością 0.1, niezerowe ≥ 0.1, suma dokładnie 100.0 (poprawka na największym). */
function percents(ws) {
  const sum = ws.reduce((a, b) => a + b, 0);
  const p = ws.map((w) => (w > 0 ? Math.max(1, Math.round((w / sum) * 1000)) : 0));
  const diff = 1000 - p.reduce((a, b) => a + b, 0);
  p[p.indexOf(Math.max(...p))] += diff;
  return p; // w promilach ×10, czyli dziesiątych procenta
}

const table = BIOMES.map((_, b) =>
  VARS.map((_, v) => percents(RES.map((r) => listed[b][v][r] ?? background(r, b, v)))),
);

// Kontrola: sumy i udział zasobów z tabeli GDD.
let minShare = 100, maxShare = 0;
table.forEach((vs, b) => vs.forEach((p, v) => {
  if (p.reduce((a, c) => a + c, 0) !== 1000) throw new Error('suma kolumny ≠ 100%');
  if (b === 12) return;
  const s = RES.reduce((a, r, i) => a + (listed[b][v][r] !== undefined ? p[i] : 0), 0) / 10;
  minShare = Math.min(minShare, s); maxShare = Math.max(maxShare, s);
}));
console.log(`udział zasobów z tabeli GDD w kolumnie: ${minShare}–${maxShare}%`);

// Rust: stała dla generatora (crates/mapgen/src/resources.rs).
const rows = table
  .map((vs, b) => `    // ${TYPES[b]} – ${BIOMES[b].toLowerCase()}\n    [\n${vs
    .map((p, v) => `        // ${VARS[v]}\n        [${p.join(', ')}],`)
    .join('\n')}\n    ],`)
  .join('\n');
const rust = `// Wygenerowane przez tools/resources/chances.mjs – nie edytuj ręcznie.

/// Liczba zasobów (\`Province::resource\` < RESOURCE_COUNT).
pub const RESOURCE_COUNT: usize = ${RES.length};

/// Numer złota (bardzo rzadkie – generator gwarantuje co najmniej jedną prowincję).
pub const GOLD: u8 = ${RES.indexOf('Złoto')};

/// Nazwy zasobów w kolejności numerów.
pub const RESOURCE_NAMES: [&str; RESOURCE_COUNT] = [
${RES.map((r) => `    "${r}",`).join('\n')}
];

/// Szanse zasobów w dziesiątych częściach procenta: \`[biom][ukształtowanie][zasób]\`, każda kolumna
/// sumuje się do 1000. Ukształtowania: płaski, wyżyna, góry, rzeki i jeziora, brzeg.
#[rustfmt::skip]
pub const RESOURCE_CHANCES: [[[u16; RESOURCE_COUNT]; 5]; ${BIOMES.length}] = [
${rows}
];
`;
writeFileSync(resolve(ROOT, 'crates/mapgen/src/resource_table.rs'), rust);

// Markdown (opcjonalnie): sekcja na biom, wiersze = zasoby, kolumny = ukształtowania; zasoby z tabeli GDD
// pogrubione. Zasoby niemożliwe w całym biomie (same zera) są pominięte.
const mdAt = process.argv.indexOf('--md');
if (mdAt > 0) {
  const fmt = (x) => (x === 0 ? '–' : (x / 10).toFixed(1).replace('.', ',') + '%');
  let md = '';
  BIOMES.forEach((name, b) => {
    md += `## ${TYPES[b]} – ${name.toLowerCase()}\n\n| Zasób | ${VARS.join(' | ')} |\n| --- |${' --- |'.repeat(5)}\n`;
    RES.forEach((r, i) => {
      if (VARS.every((_, v) => table[b][v][i] === 0)) return;
      const cells = VARS.map((_, v) => {
        const s = fmt(table[b][v][i]);
        return listed[b][v][r] !== undefined ? `**${s}**` : s;
      });
      md += `| ${r} | ${cells.join(' | ')} |\n`;
    });
    md += '\n';
  });
  writeFileSync(process.argv[mdAt + 1], md);
}
