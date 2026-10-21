import { Component, computed, inject } from '@angular/core';

import type { LedgerReport } from '../../generated/LedgerReport';
import type { NationReport } from '../../generated/NationReport';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { FloatingWindow } from './floating-window';
import { amount, full, short } from './format';

/** Złoto w interfejsie jest dzielone przez tyle (jak w pasku zasobów). */
const GOLD_UNIT = 10;
/** Dni w miesiącu gry (podatek co 300 tur po 10 na dzień). */
const DAYS_PER_MONTH = 30;
/** Punkty wykresu przyrostu: krzywa x · (1 − x) / (1 + 1,25x) (szczyt przy 40% – `economy::growth`), znormalizowana. */
const CURVE_STEPS = 40;
const curveAt = (x: number) => (x * (1 - x)) / (1 + 1.25 * x) / 0.16;

/** Wiersz księgi złota: źródło, kwota (wydatki ujemne), opis; `forecast` – w bieżącym miesiącu prognoza. */
interface LedgerRow {
  label: string;
  now: number;
  last: number;
  title: string;
  forecast?: boolean;
}

/**
 * Wiersze księgi: przychody (+) i wydatki (−) w bieżącym i poprzednim miesiącu. Podatek wpływa na koniec
 * miesiąca, więc w bieżącym – prognoza najbliższego (`nextTax`, złoto z interfejsu).
 */
function ledgerRows(now: LedgerReport, last: LedgerReport, nextTax: number, upkeep: number): LedgerRow[] {
  return [
    { label: 'Podatek', now: nextTax, last: last.tax, title: 'Podatek od robotników na koniec miesiąca (w bieżącym – prognoza)', forecast: true },
    { label: 'Wydobycie złota', now: now.mining, last: last.mining, title: 'Kopalnie złota – prosto do skarbca' },
    { label: 'Łupy', now: now.loot, last: last.loot, title: 'Połowa skarbca przyłączonych państw' },
    { label: 'Rynek – sprzedaż', now: now.sales, last: last.sales, title: 'Eksport surowców (gracze i handlarze spoza mapy)' },
    { label: 'Cło tranzytowe', now: now.duties, last: last.duties, title: 'Cło od cudzych dostaw przez twoje terytorium' },
    { label: 'Rynek – zakupy', now: -now.purchases, last: -last.purchases, title: 'Import surowców z kosztem dostawy' },
    { label: 'Budowy', now: -now.buildings, last: -last.buildings, title: 'Budowy i rozbudowy (minus zwroty za anulowanie)' },
    { label: 'Stolica', now: -now.capital, last: -last.capital, title: 'Przeniesienia stolicy' },
    {
      label: 'Utrzymanie wojska',
      now: -upkeep,
      last: -last.army,
      title: 'Co miesiąc z podatkiem: 0,1 złota od żołnierza, ponad miękkim limitem wojska coraz więcej (koszary podnoszą limit)',
      forecast: true,
    },
  ];
}

/** Kwota ze znakiem, np. „+1,2k” / „−500”. */
function signed(v: number): string {
  if (v === 0) return '–';
  return (v > 0 ? '+' : '−') + short(Math.abs(v));
}

/** Procent z ułamka, np. 0,87 → „87%”. */
const pct = (v: number) => `${Math.round(v * 100)}%`;

/**
 * Okno państwa (kliknięcie złota albo populacji w pasku zasobów): finanse – podatek z mnożnikami, księga
 * złota z bieżącego i poprzedniego miesiąca (import i eksport dojdą z rynkiem) – oraz ludność: limit
 * z kafli i miast, przyrost logistyczny z mnożnikiem prowiantu i głodem, krzywa przyrostu z bieżącym
 * zapełnieniem.
 */
@Component({
  selector: 'app-nation-window',
  imports: [FloatingWindow],
  template: `
    @if (store.nationWindow() !== 'closed' && view(); as v) {
      <app-floating-window title="Państwo" key="mapa.nationWindow.pos" [width]="340" [initial]="{ x: 420, y: 72 }" (closed)="store.nationWindow.set('closed')">
        <nav class="tabs" role="tablist">
          <button type="button" role="tab" [class.active]="store.nationWindow() === 'finance'" (click)="store.nationWindow.set('finance')">Finanse</button>
          <button type="button" role="tab" [class.active]="store.nationWindow() === 'population'" (click)="store.nationWindow.set('population')">Ludność</button>
        </nav>
        @if (store.nationWindow() === 'finance') {
          <dl class="sum">
            <dt>Skarbiec</dt>
            <dd class="gold">●{{ v.gold }}</dd>
          </dl>
          <h4>Najbliższy podatek</h4>
          <dl class="sum">
            <dt [title]="'Robotnicy × ' + v.rate + ' złota (wojsko nie płaci)'">Robotnicy {{ v.workers }} × {{ v.rate }}</dt>
            <dd>{{ v.taxBase }}</dd>
            <dt title="Średnie pokrycie podstawowego zaopatrzenia cywilnego z miesiąca: 50% podatku bez niego, 100% przy pełnym">× Podstawowe cywilne</dt>
            <dd [class.low]="v.basicLow">{{ v.basicTax }}</dd>
            <dt title="Średnie pokrycie luksusowego z miesiąca – do +50%, tylko przy pełnym podstawowym">+ Luksusowe cywilne</dt>
            <dd>+{{ v.luxury }}</dd>
            <dt class="total">= Podatek</dt>
            <dd class="total gold">●{{ v.nextTax }}</dd>
          </dl>
          <h4>Księga złota</h4>
          <table class="ledger">
            <thead><tr><th></th><th title="Od ostatniego podatku; podatek – prognoza">ten miesiąc</th><th>poprzedni</th></tr></thead>
            <tbody>
              @for (r of v.ledger; track r.label) {
                <tr [title]="r.title"><td>{{ r.label }}</td><td [class.minus]="r.now < 0">{{ r.nowText }}</td><td [class.minus]="r.last < 0">{{ r.lastText }}</td></tr>
              }
              <tr class="muted" title="Handel surowcami – razem z rynkiem (etap 8)"><td>Import / eksport</td><td>–</td><td>–</td></tr>
              <tr class="total"><td>Bilans</td><td [class.minus]="v.balanceNow < 0">{{ v.balanceNowText }}</td><td [class.minus]="v.balanceLast < 0">{{ v.balanceLastText }}</td></tr>
            </tbody>
          </table>
          <p class="hint">Wydobycie złota teraz: {{ v.miningMonth }} na miesiąc.</p>
        } @else {
          <dl class="sum">
            <dt>Populacja</dt>
            <dd><b>{{ v.population }}</b> / {{ v.cap }}</dd>
            <dt>Limit: kafle + miasta</dt>
            <dd>{{ v.capTiles }} + {{ v.capCities }}</dd>
            <dt>Wojsko · w bitwie · robotnicy</dt>
            <dd>{{ v.troops }} · {{ v.deployed }} · {{ v.workersShort }}</dd>
          </dl>
          <h4>Przyrost na dzień</h4>
          <dl class="sum">
            <dt title="Krzywa S: najszybciej przy 40% limitu, coraz wolniej przy limicie">Przyrost (zapełnienie {{ v.fill }})</dt>
            <dd>{{ v.base }}</dd>
            <dt title="Prowiant: przyrost × do 150% przy pełnym pokryciu, 0% bez prowiantu">× Prowiant (pokrycie {{ v.food }})</dt>
            <dd [class.low]="v.foodLow">{{ v.foodPct }}</dd>
            <dt title="Bez prowiantu populacji ubywa do 0,5% dziennie">− Głód</dt>
            <dd [class.minus]="v.starving">{{ v.starve }}</dd>
            <dt class="total">= Zmiana</dt>
            <dd class="total" [class.minus]="v.netNegative">{{ v.net }}</dd>
          </dl>
          <svg class="curve" viewBox="0 0 200 78" role="img" [attr.aria-label]="'Krzywa przyrostu, zapełnienie ' + v.fill">
            <path class="axis" d="M4 64H196M4 64V4" />
            <path class="line" [attr.d]="curve" />
            <line class="now" [attr.x1]="v.markX" [attr.x2]="v.markX" y1="4" y2="64" />
            <circle class="dot" [attr.cx]="v.markX" [attr.cy]="v.markY" r="3.5" />
            <text x="6" y="12">przyrost</text>
            <text x="196" y="76" text-anchor="end">zapełnienie limitu →</text>
          </svg>
        }
      </app-floating-window>
    }
  `,
  styles: `
    .tabs { display: flex; gap: 4px; margin-bottom: 8px; }
    .tabs button { flex: 1; padding: 3px 6px; font-size: 12px; }
    .tabs button.active { color: #fff; background: #2f6f55; border-color: #3f8a6b; }
    h4 { margin: 10px 0 4px; color: #8ea3b4; font-size: 11px; font-weight: 600; letter-spacing: 0.05em; text-transform: uppercase; }
    .sum { display: grid; grid-template-columns: 1fr auto; gap: 3px 12px; margin: 0; font-size: 12px; }
    .sum dt { color: #b8c6d2; }
    .sum dd { margin: 0; text-align: right; font-variant-numeric: tabular-nums; }
    .sum .total { padding-top: 3px; border-top: 1px solid #243847; font-weight: 600; color: #fff; }
    .gold { color: #f2d47a; }
    .low { color: #e8cc72; }
    .minus { color: #ee8f82; }
    .ledger { width: 100%; border-collapse: collapse; font-size: 12px; font-variant-numeric: tabular-nums; }
    .ledger th { color: #8ea3b4; font-weight: 400; font-size: 11px; text-align: right; }
    .ledger td { padding: 2px 0; text-align: right; }
    .ledger td:first-child { text-align: left; color: #b8c6d2; }
    .ledger tr.total td { padding-top: 4px; border-top: 1px solid #243847; font-weight: 600; color: #fff; }
    .ledger tr.muted td { color: #5f7384; }
    .hint { margin: 8px 0 0; color: #8ea3b4; font-size: 11px; }
    .curve { width: 100%; margin-top: 8px; }
    .curve .axis { fill: none; stroke: #3a5266; stroke-width: 1; }
    .curve .line { fill: none; stroke: #6fbf9a; stroke-width: 2; }
    .curve .now { stroke: #f2d47a; stroke-width: 1; stroke-dasharray: 3 3; }
    .curve .dot { fill: #f2d47a; }
    .curve text { fill: #8ea3b4; font-size: 9px; }
  `,
})
export class NationWindow {
  protected readonly store = inject(MapStore);
  private readonly session = inject(GameSession);

  /** Krzywa przyrostu na wykresie 192 × 56. */
  protected readonly curve = Array.from({ length: CURVE_STEPS + 1 }, (_, k) => {
    const x = k / CURVE_STEPS;
    return `${k === 0 ? 'M' : 'L'}${(4 + x * 192).toFixed(1)} ${(64 - curveAt(x) * 56).toFixed(1)}`;
  }).join('');

  protected readonly view = computed(() => {
    const e = this.session.myEconomy();
    return e ? { ...this.finance(e), ...this.people(e) } : null;
  });

  private finance(e: NationReport) {
    const g = (v: number) => Math.floor(v / GOLD_UNIT);
    const rows = ledgerRows(e.month, e.lastMonth, g(e.nextTax), g(e.upkeep)).map((r) => ({
      ...r,
      nowText: (r.forecast && r.now > 0 ? '≈' : '') + signed(r.now),
      lastText: signed(r.last),
    }));
    const balance = (k: 'now' | 'last') => rows.reduce((s, r) => s + r[k], 0);
    return {
      gold: full(g(e.gold)),
      workers: short(e.workers),
      rate: amount(e.taxPer1000 / GOLD_UNIT / 1000),
      taxBase: full(g(e.taxBase)),
      basicTax: pct(e.basicTax),
      basicLow: e.basicTax < 1,
      luxury: pct(e.taxBonus),
      nextTax: full(g(e.nextTax)),
      ledger: rows,
      balanceNow: balance('now'),
      balanceLast: balance('last'),
      balanceNowText: signed(balance('now')),
      balanceLastText: signed(balance('last')),
      miningMonth: `●${full((e.goldMining / GOLD_UNIT) * DAYS_PER_MONTH)}`,
    };
  }

  private people(e: NationReport) {
    const x = e.maxPopulation > 0 ? Math.min(1, e.population / e.maxPopulation) : 0;
    const net = e.growthPerSecond;
    return {
      population: full(e.population),
      cap: full(e.maxPopulation),
      capTiles: short(e.capTiles),
      capCities: short(e.capCities),
      troops: short(e.troops),
      deployed: short(e.deployed),
      workersShort: short(e.workers),
      fill: pct(x),
      base: `${e.growthBasePerDay >= 0 ? '+' : '−'}${full(Math.abs(e.growthBasePerDay))}`,
      food: pct(e.foodCoverage),
      foodPct: pct(e.growthFoodPct),
      foodLow: e.foodCoverage < 1,
      starve: e.starvePerDay > 0 ? `−${full(e.starvePerDay)}` : '–',
      starving: e.starvePerDay > 0,
      net: `${net >= 0 ? '+' : '−'}${full(Math.abs(net))}`,
      netNegative: net < 0,
      markX: (4 + x * 192).toFixed(1),
      markY: (64 - curveAt(x) * 56).toFixed(1),
    };
  }
}
