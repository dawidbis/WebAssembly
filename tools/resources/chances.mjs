// Tabela szans zasobów: biom × ukształtowanie → rozkład na zasoby (suma 100%).
// Wagi: zasoby dominujące biomu (tożsamość biomów – docs/gdd/balans.md) + tło z reguł (klimat dla zasobów
// biologicznych, geologia dla kopalin – kopaliny nie zależą od klimatu). Zero = wyjątek (zasób niemożliwy).
// Na końcu każda waga razy skala zasobu (`scale.json`) – dobrana kalibracją do docelowego udziału
// w prowincjach (`TARGET`).
//
//   node tools/resources/chances.mjs                 → crates/mapgen/src/resource_table.rs
//   node tools/resources/chances.mjs --md plik       → dodatkowo tabele w markdown (do dokumentu GDD)
//   node tools/resources/chances.mjs --calibrate 4   → 4 rundy: tabela, raport `--stats` z CLI generatora,
//                                                       poprawka skal w scale.json (potem zbuduj i sprawdź)
//
// Kolejność zasobów (RES) = numery `Province.resource`; ta sama lista z kolorami jest w
// web/src/app/render/resources.ts – zmieniaj obie. Tabele w rdzeniu (`goods::EXTRACTION`, `BASE_PRICE`)
// i w generatorze (`slots::FIT`) idą w tej kolejności, testy pilnują nazw.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const SCALE_FILE = resolve(ROOT, 'tools/resources/scale.json');

const BIOMES = ['Las deszczowy', 'Sawanna', 'Pustynia', 'Step', 'Śródziemnomorski', 'Subtropikalny', 'Oceaniczny',
  'Gorące lato', 'Ciepłe lato', 'Borealny', 'Tajga', 'Tundra', 'Lądolód'];
const TYPES = ['Tropikalny', 'Tropikalny', 'Suchy', 'Suchy', 'Umiarkowany', 'Umiarkowany', 'Umiarkowany',
  'Kontynentalny', 'Kontynentalny', 'Kontynentalny', 'Polarny', 'Polarny', 'Polarny'];
const VARS = ['Płaski', 'Wyżyna', 'Góry', 'Rzeki i jeziora', 'Brzeg'];
const DRY = new Set([2, 3]);

// Zasoby w kolejności numerów (grupy jak w legendzie) z nazwą stałej w Ruście (`resources::id`) i docelowym
// udziałem w prowincjach (%, suma 100; złoto – bez kalibracji, generator gwarantuje co najmniej jedno).
const RESOURCES = [
  // Rolnictwo: skrobia na prowiant, włókna, używki, kauczuk.
  ['Zboża', 'GRAIN', 4.5], ['Ryż', 'RICE', 2.5], ['Kukurydza', 'CORN', 2.4], ['Ziemniaki', 'POTATOES', 2.0],
  ['Bawełna', 'COTTON', 4.5], ['Włókna roślinne', 'FIBERS', 2.3], ['Tytoń', 'TOBACCO', 2.5],
  ['Trzcina cukrowa', 'SUGAR', 2.5], ['Kawa', 'COFFEE', 0.35], ['Herbata', 'TEA', 0.35], ['Kauczuk', 'RUBBER', 2.3],
  // Hodowla i połowy: białko na prowiant, konie, wełna, futra.
  ['Bydło', 'CATTLE', 3.0], ['Trzoda chlewna', 'PIGS', 2.0], ['Drób', 'POULTRY', 1.5], ['Ryby', 'FISH', 2.0],
  ['Konie', 'HORSES', 2.8], ['Wełna', 'WOOL', 2.3], ['Futra', 'FURS', 1.8],
  // Las.
  ['Drewno', 'WOOD', 7.5],
  // Energia.
  ['Węgiel kamienny', 'COAL', 4.5], ['Węgiel brunatny', 'LIGNITE', 3.4], ['Ropa naftowa', 'OIL', 5.2],
  // Metale.
  ['Rudy żelaza', 'IRON', 8.5], ['Mangan', 'MANGANESE', 2.7], ['Chrom', 'CHROME', 2.4], ['Nikiel', 'NICKEL', 1.7],
  ['Wolfram', 'TUNGSTEN', 1.2], ['Miedź', 'COPPER', 4.0], ['Ołów', 'LEAD', 2.4], ['Aluminium', 'ALUMINIUM', 1.5],
  ['Cyna', 'TIN', 1.8], ['Złoto', 'GOLD', null],
  // Chemia.
  ['Surowce nawozowe', 'FERTILIZER', 2.5], ['Cement', 'CEMENT', 4.8], ['Saletra', 'SALTPETER', 2.2],
  ['Siarka', 'SULFUR', 1.0], ['Sól', 'SALT', 1.0],
];
const RES = RESOURCES.map((r) => r[0]);
const TARGET = Object.fromEntries(RESOURCES.filter((r) => r[2] !== null).map((r) => [r[0], r[2]]));

// Zasoby dominujące (tożsamość biomów, realia dwudziestolecia): [biom][wariant] = {zasób: waga}.
// Kolumny: płaski | wyżyna | góry | rzeki i jeziora | brzeg.
const U = `
0|Kauczuk 100, Trzcina cukrowa 70, Ryż 30|Aluminium 90, Cyna 50, Kauczuk 50, Kawa 35, Herbata 15|Aluminium 100, Cyna 80, Mangan 50, Rudy żelaza 30|Ryż 70, Kauczuk 50, Cyna 50|Trzcina cukrowa 60, Kauczuk 40, Ryby 30
1|Bawełna 80, Bydło 70, Kukurydza 40, Konie 30|Mangan 70, Chrom 60, Rudy żelaza 50, Bydło 30, Kawa 15|Mangan 90, Chrom 90, Rudy żelaza 70, Cyna 40, Miedź 40|Ryż 50, Bawełna 50, Trzcina cukrowa 50, Bydło 40|Trzcina cukrowa 40, Ryby 30, Sól 20
2|Ropa naftowa 100, Saletra 60, Surowce nawozowe 50, Sól 40|Ropa naftowa 70, Miedź 70, Saletra 50, Surowce nawozowe 30, Sól 30|Miedź 100, Chrom 70, Saletra 60, Siarka 60, Sól 40, Cyna 30|Bawełna 30, Ryż 30, Trzcina cukrowa 20, Kukurydza 20|Ropa naftowa 60, Sól 60, Ryby 20
3|Konie 70, Bydło 60, Zboża 50, Bawełna 30|Wełna 70, Miedź 70, Konie 40, Rudy żelaza 40|Miedź 100, Chrom 60, Saletra 50, Wolfram 30, Siarka 30|Zboża 60, Bydło 40, Bawełna 40|Sól 40, Ropa naftowa 40, Ryby 15
4|Tytoń 80, Zboża 40, Bawełna 40|Cement 80, Tytoń 50, Ołów 50, Wełna 40|Cement 90, Ołów 80, Miedź 60, Siarka 50|Ryż 50, Zboża 40, Kukurydza 30|Ryby 60, Sól 50, Tytoń 30
5|Ryż 60, Kukurydza 50, Zboża 40, Tytoń 30|Rudy żelaza 80, Cement 60, Węgiel kamienny 50, Herbata 35|Rudy żelaza 90, Węgiel kamienny 70, Ołów 60, Wolfram 50|Ryż 80, Trzoda chlewna 30|Ryby 50, Ryż 30
6|Zboża 50, Ziemniaki 40, Wełna 40, Trzoda chlewna 30|Cement 70, Wełna 60, Węgiel kamienny 60, Rudy żelaza 50|Węgiel kamienny 90, Rudy żelaza 70, Ołów 60, Cyna 40|Drewno 50, Włókna roślinne 50, Ziemniaki 30|Ryby 80, Wełna 30
7|Kukurydza 70, Trzoda chlewna 50, Zboża 50, Konie 30|Rudy żelaza 80, Węgiel kamienny 70, Cement 60|Węgiel kamienny 90, Rudy żelaza 90, Mangan 60|Kukurydza 50, Trzoda chlewna 40, Włókna roślinne 40|Ryby 40
8|Zboża 60, Ziemniaki 50, Włókna roślinne 40, Drewno 30|Węgiel brunatny 70, Cement 60, Mangan 50|Mangan 80, Rudy żelaza 70, Węgiel kamienny 70, Węgiel brunatny 60|Drewno 50, Włókna roślinne 50, Ziemniaki 40|Ryby 50
9|Drewno 70, Ziemniaki 30, Futra 15|Węgiel brunatny 70, Drewno 40, Nikiel 40|Nikiel 90, Rudy żelaza 70, Węgiel brunatny 50, Wolfram 30|Drewno 60, Ryby 30|Ryby 70, Drewno 30
10|Drewno 70, Futra 30|Węgiel brunatny 40, Futra 30, Wolfram 30, Nikiel 30|Wolfram 90, Nikiel 80, Chrom 50|Drewno 50, Ryby 30, Futra 20|Ryby 80, Futra 20
11|Futra 40, Węgiel brunatny 40|Węgiel brunatny 50, Nikiel 40, Futra 20|Nikiel 90, Wolfram 80, Chrom 40|Ryby 50, Futra 30|Ryby 90, Futra 20`;
const listed = BIOMES.map(() => VARS.map(() => ({})));
for (const line of U.trim().split('\n')) {
  const [b, ...vs] = line.split('|');
  vs.forEach((v, i) => {
    for (const item of v.split(',')) {
      if (!item.trim()) continue;
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
  'Kukurydza': [0.6, 1, 0.1, 0.8, 0.8, 1, 0.3, 1, 0.6, 0.1, 0, 0, 0],
  'Ziemniaki': [0, 0.1, 0, 0.3, 0.3, 0.4, 1, 0.5, 1, 0.8, 0.5, 0.1, 0],
  'Bawełna': [0.3, 1, 0.3, 0.8, 1, 0.8, 0, 0.5, 0, 0, 0, 0, 0],
  'Włókna roślinne': [0.2, 0.3, 0.1, 0.5, 0.5, 0.6, 0.8, 0.7, 1, 0.6, 0.3, 0, 0],
  'Tytoń': [0.6, 0.6, 0.1, 0.3, 1, 1, 0.1, 0.7, 0.2, 0, 0, 0, 0],
  'Trzcina cukrowa': [1, 0.8, 0.2, 0, 0.3, 0.6, 0, 0.1, 0, 0, 0, 0, 0],
  'Kawa': [1, 0.6, 0, 0, 0.1, 0.3, 0, 0, 0, 0, 0, 0, 0],
  'Herbata': [0.7, 0.2, 0, 0, 0.2, 1, 0.2, 0.1, 0, 0, 0, 0, 0],
  'Kauczuk': [1, 0.3, 0, 0, 0, 0.1, 0, 0, 0, 0, 0, 0, 0],
  'Bydło': [0.2, 1, 0.2, 1, 0.6, 1, 1, 1, 0.9, 0.4, 0.1, 0, 0],
  'Trzoda chlewna': [0.4, 0.5, 0.1, 0.5, 0.8, 1, 1, 1, 1, 0.7, 0.3, 0, 0],
  'Drób': [0.6, 0.7, 0.3, 0.7, 1, 1, 1, 1, 1, 0.6, 0.3, 0.1, 0],
  'Ryby': [0.6, 0.6, 0.4, 0.5, 0.8, 0.8, 1, 0.7, 0.9, 1, 1, 1, 0.6],
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
const FISH = [0, 0, 0, 0.8, 3]; // tylko wody śródlądowe i brzeg
const BIO_TERRAIN = {
  'Zboża': CROP, 'Ryż': CROP, 'Kukurydza': CROP, 'Ziemniaki': CROP, 'Bawełna': CROP, 'Włókna roślinne': CROP,
  'Tytoń': CROP, 'Trzcina cukrowa': CROP, 'Kawa': HIGH_CROP, 'Herbata': HIGH_CROP, 'Kauczuk': CROP,
  'Bydło': HERD, 'Trzoda chlewna': HERD, 'Drób': HERD, 'Ryby': FISH, 'Konie': HERD, 'Wełna': SHEEP, 'Futra': FUR,
  'Drewno': WOOD,
};
const BIO_BASE = 8;

// Kopaliny: waga bazowa i ukształtowanie (geologia, bez zależności od klimatu).
const ORE = [0.25, 0.6, 1, 0.15, 0.15];
const MINERAL = {
  'Rudy żelaza': [6, ORE], 'Węgiel kamienny': [5, ORE], 'Węgiel brunatny': [5, [0.6, 1, 0.6, 0.4, 0.3]],
  'Miedź': [4, ORE], 'Mangan': [3, ORE], 'Chrom': [3, ORE], 'Ołów': [3, ORE],
  'Nikiel': [2, ORE], 'Aluminium': [2, ORE], 'Cyna': [2, [0.25, 0.6, 1, 0.5, 0.15]], 'Wolfram': [1.5, ORE],
  // Złoto: bardzo rzadkie, tylko złoża kruszcowe w górach i złoto okruchowe w rzekach (zwykle 1–2 prowincje na mapę,
  // generator gwarantuje co najmniej jedną – `resources.rs`).
  'Złoto': [1, [0, 0, 1, 0.4, 0]],
  'Ropa naftowa': [4, [1, 0.5, 0.1, 0.5, 1]],
  'Surowce nawozowe': [3, [1, 1, 0.5, 0.6, 0.6]],
  // Wapień jest pospolity – cement w każdym biomie.
  'Cement': [5, [0.6, 1, 1, 0.5, 0.5]],
  'Saletra': [2, [0.6, 0.6, 1, 0.3, 0.3]],
  'Siarka': [2, [0.3, 0.6, 1, 0.3, 0.3]],
  // Sól: złoża kamienne w górach i warzelnie na suchych wybrzeżach.
  'Sól': [1, [0.3, 0.5, 1, 0.3, 0.8]],
};
const TROPICAL = new Set([0, 1]);

function background(r, b, v) {
  if (SUIT[r]) return BIO_BASE * SUIT[r][b] * BIO_TERRAIN[r][v];
  const [base, terr] = MINERAL[r];
  let w = base * terr[v];
  // Boksyt (ruda aluminium) to zwietrzelina laterytowa, cyna – złoża okruchowe: głównie tropiki.
  if (r === 'Aluminium' || r === 'Cyna') w *= TROPICAL.has(b) ? 2 : 0.5;
  if ((r === 'Surowce nawozowe' || r === 'Saletra' || r === 'Siarka' || r === 'Sól') && DRY.has(b)) w *= 1.5;
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

const readScale = () => {
  const saved = existsSync(SCALE_FILE) ? JSON.parse(readFileSync(SCALE_FILE, 'utf8')) : {};
  return Object.fromEntries(RES.map((r) => [r, saved[r] ?? 1]));
};

function buildTable(scale) {
  return BIOMES.map((_, b) =>
    VARS.map((_, v) => percents(RES.map((r) => (listed[b][v][r] ?? background(r, b, v)) * scale[r]))),
  );
}

function writeRust(table) {
  // Kontrola: sumy kolumn.
  table.forEach((vs) => vs.forEach((p) => {
    if (p.reduce((a, c) => a + c, 0) !== 1000) throw new Error('suma kolumny ≠ 100%');
  }));
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

/// Numery zasobów (\`Province::resource\`) – kolejność \`RESOURCE_NAMES\`.
pub mod id {
${RESOURCES.map(([name, id], i) => `    /// ${name}.\n    pub const ${id}: u8 = ${i};`).join('\n')}
}

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
}

/** Udziały zasobów (%) z raportu `--stats` CLI generatora. */
function measure() {
  const out = execFileSync(
    'cargo',
    ['run', '-q', '-p', 'game-mapgen', '--release', '--features', 'cli', '--', '--stats', '6', '--seed', '1'],
    { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] },
  );
  const shares = {};
  const section = out.split('Surowce:')[1].split('Biomy')[0];
  for (const line of section.split('\n')) {
    const m = line.trim().match(/^(.*?)\s+([\d.]+)%$/);
    if (m) shares[m[1]] = +m[2];
  }
  return shares;
}

const scale = readScale();
const calibrateAt = process.argv.indexOf('--calibrate');
if (calibrateAt > 0) {
  const rounds = +(process.argv[calibrateAt + 1] ?? 3);
  for (let k = 0; k < rounds; k++) {
    writeRust(buildTable(scale));
    const shares = measure();
    let worst = 0;
    for (const r of Object.keys(TARGET)) {
      const got = shares[r] ?? 0;
      worst = Math.max(worst, Math.abs(got - TARGET[r]) / TARGET[r]);
      scale[r] *= got > 0 ? (TARGET[r] / got) ** 0.9 : 2;
      scale[r] = +scale[r].toFixed(4);
    }
    console.log(`runda ${k + 1}: największy błąd ${(worst * 100).toFixed(0)}%`);
    writeFileSync(SCALE_FILE, JSON.stringify(scale, null, 2) + '\n');
  }
}
const table = buildTable(scale);
writeRust(table);

// Markdown (opcjonalnie): sekcja na biom, wiersze = zasoby, kolumny = ukształtowania; zasoby dominujące
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
