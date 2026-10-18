import { Component, computed, effect, inject, signal } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { flagDataUrl } from '../render/flags';
import { FloatingWindow } from './floating-window';
import { amount, full, short } from './format';
import { GoodsIcon } from './goods-icon';
import type { MarketResourceReport } from '../../generated/MarketResourceReport';

/** Zapamiętane ustawienia okna (zakładka, rozwinięte grupy) – tylko wygoda. */
const PREFS_KEY = 'mapa.marketWindow';
type Tab = 'balance' | 'prices' | 'deals';
/** Sortowanie zakładki „Ceny”: kolumna i kierunek (1 – rosnąco). */
type SortKey = 'name' | 'price' | 'change' | 'share';
interface Sort {
  key: SortKey;
  dir: 1 | -1;
}

interface Prefs {
  tab: Tab;
  open: string[];
  sort: Sort;
}

const BY: Record<SortKey, (a: MarketResourceReport, b: MarketResourceReport) => number> = {
  name: (a, b) => a.name.localeCompare(b.name, 'pl'),
  price: (a, b) => a.price - b.price,
  change: (a, b) => a.price / a.prevPrice - b.price / b.prevPrice,
  share: (a, b) => a.share - b.share,
};

function loadPrefs(): Partial<Prefs> {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    return raw ? (JSON.parse(raw) as Partial<Prefs>) : {};
  } catch {
    return {};
  }
}

/** Bilans z ilością dziennie: „import 50% · 2,5/d”. */
function balanceLine(setting: number, capacity: number): string {
  if (setting === 0) return 'bez handlu';
  return `${balanceText(setting)} · ${amount((capacity * Math.abs(setting)) / 100)}/d`;
}

/** Bilans słownie: „import 50%”, „eksport 100%”, „bez handlu”. */
export function balanceText(setting: number): string {
  if (setting === 0) return 'bez handlu';
  return `${setting < 0 ? 'import' : 'eksport'} ${Math.abs(setting)}%`;
}

/** Kwota złota: małe z jednym miejscem po przecinku (0,4), duże w całości (12 500). */
function money(v: number): string {
  return v < 100 ? amount(v) : full(Math.round(v));
}

/** Cena z interfejsu, np. „16,2”. */
function price(v: number): string {
  return v.toFixed(1).replace('.', ',');
}

/**
 * Co automat zrobi z surowcem (i dlaczego nic): przepustowość × bilans, zapas, port – automat nie może być
 * czarną skrzynką. `setting` – bilans (%), `capacity` – przepustowość (jednostki na dzień).
 */
function reason(r: MarketResourceReport, setting: number, capacity: number, traders: boolean): string {
  const perDay = (capacity * Math.abs(setting)) / 100;
  const days = r.need > 0 ? `, zapas na ${Math.floor(r.stock / r.need)} dni potrzeby fabryk` : '';
  if (setting === 0) return r.need > 0 ? `Fabryki potrzebują ${amount(r.need)}/d${days}` : '';
  if (capacity <= 0) return 'Przepustowość 0 – zbuduj port albo fabrykę';
  const sellers = traders ? '' : ' (bez portu – tylko od graczy z trasą lądem)';
  if (setting < 0) return `Kupuje do ${amount(perDay)}/d (${Math.abs(setting)}% przepustowości)${sellers}${days}`;
  if (r.stock <= 0) return 'Brak zapasu do sprzedaży';
  return `Sprzedaje do ${amount(Math.min(perDay, r.stock))}/d (${setting}% przepustowości, najwyżej zapas)${traders ? '' : ' – bez portu tylko graczom'}`;
}

/**
 * Okno rynku (przycisk z wagą w pasku zasobów): bilans import/eksport grup surowców z nadpisaniem dla
 * pojedynczego surowca, ceny z trendem i wczorajsze transakcje (z kim, ile, za ile, koszt dostawy).
 */
@Component({
  selector: 'app-market-window',
  imports: [FloatingWindow, GoodsIcon],
  templateUrl: './market-window.html',
  styleUrls: ['./ui.css', './market-window.css'],
})
export class MarketWindow {
  protected readonly store = inject(MapStore);
  protected readonly session = inject(GameSession);
  private readonly saved = loadPrefs();
  protected readonly tab = signal<Tab>(this.saved.tab ?? 'balance');
  protected readonly open = signal(new Set(this.saved.open ?? []));
  protected readonly sort = signal<Sort>(this.saved.sort ?? { key: 'name', dir: 1 });
  /** Suwaki w trakcie przeciągania (zanim wróci raport z nowym ustawieniem). */
  protected readonly resourceDraft = signal<Record<number, number>>({});
  protected readonly initial = { x: Math.max(8, Math.round(innerWidth / 2 - 250)), y: 96 };
  protected readonly balanceText = balanceText;

  protected readonly view = computed(() => {
    const m = this.session.market();
    if (!m) return null;
    const byId = new Map(m.resources.map((r) => [r.resource, r]));
    const resourceDraft = this.resourceDraft();
    const traders = m.traderDelivery !== null;
    const nations = this.session.owners()?.nations ?? [];
    const who = (player: number | null) => {
      if (player === null) return { name: 'Handlarze spoza mapy', flag: '' };
      const nation = nations.indexOf(player) + 1;
      return { name: this.session.playerName(player), flag: nation > 0 ? flagDataUrl(nation, 18, 12) : '' };
    };
    const row = (r: MarketResourceReport) => {
      const change = r.prevPrice > 0 ? (r.price - r.prevPrice) / r.prevPrice : 0;
      return {
        ...r,
        priceText: price(r.price),
        trend: change > 0.0005 ? 'up' : change < -0.0005 ? 'down' : '',
        trendText: Math.abs(change) < 0.0005 ? '–' : `${change > 0 ? '+' : '−'}${Math.abs(change * 100).toFixed(1).replace('.', ',')}%`,
        demandText: `${amount(r.demand)} / ${amount(r.supply)}`,
        vsBase: Math.round((r.price / r.basePrice) * 100),
        stockText: amount(r.stock),
        needText: r.need > 0 ? `${amount(r.need)}/d` : '',
        reason: reason(r, resourceDraft[r.resource] ?? r.setting, m.capacity, traders),
        balance: balanceLine(resourceDraft[r.resource] ?? r.setting, m.capacity),
        slider: resourceDraft[r.resource] ?? r.setting,
        shareText: r.share > 0 ? `${Math.round(r.share * 100)}%` : '–',
        turnoverText: r.turnover > 0 ? short(r.turnover) : '',
      };
    };
    const { key, dir } = this.sort();
    return {
      groups: m.groups.map((g, index) => {
        const items = g.resources.map((id) => byId.get(id)).filter((r) => r !== undefined).map(row);
        const imports = items.filter((r) => r.slider < 0).length;
        const exports = items.filter((r) => r.slider > 0).length;
        const active = [imports ? `import ${imports}` : '', exports ? `eksport ${exports}` : ''].filter(Boolean).join(' · ');
        return { index, name: g.name, open: this.open().has(g.name), items, active };
      }),
      capacity: amount(m.capacity),
      capacityTitle: `Przepustowość handlu: porty ${amount(m.portCapacity)}/d (5 na poziom × obsada) + fabryki ${amount(m.capacity - m.portCapacity)}/d (1 na poziom). Każdy surowiec osobno: dziennie przepustowość × bilans.`,
      traders,
      prices: [...m.resources].sort((a, b) => dir * BY[key](a, b) || a.resource - b.resource).map(row),
      deals: m.deals.map((d) => ({
        ...d,
        name: byId.get(d.resource)?.name ?? '',
        partner: who(d.partner),
        amountText: amount(d.amount),
        valueText: money(d.value),
        deliveryText: d.delivery > 0 ? money(d.delivery) : '',
        transit: d.transit.map((t) => this.session.playerName(t)).join(', '),
      })),
      totals: {
        sold: short(m.deals.filter((d) => !d.buy).reduce((s, d) => s + d.value, 0)),
        bought: short(m.deals.filter((d) => d.buy).reduce((s, d) => s + d.value + d.delivery, 0)),
      },
      turnover: short(m.turnover),
      turnoverTitle: `Obrót całego rynku wczoraj: ●${full(Math.round(m.turnover))} (sprzedaż graczy graczom i handlarzom, zakupy u handlarzy). Twój udział w rynku: ${(m.share * 100).toFixed(1).replace('.', ',')}%`,
      trader: m.traderDelivery === null ? '' : `${Math.round(m.traderDelivery * 100)}%`,
    };
  });

  constructor() {
    effect(() => {
      const prefs: Prefs = { tab: this.tab(), open: [...this.open()], sort: this.sort() };
      try {
        localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
      } catch {
        // Brak dostępu do storage – ustawienia tylko na tę sesję.
      }
    });
    effect(() => {
      const m = this.session.market();
      const draft = this.resourceDraft();
      if (!m || Object.keys(draft).length === 0) return;
      const setting = new Map(m.resources.map((r) => [r.resource, r]));
      const left = Object.fromEntries(Object.entries(draft).filter(([r, v]) => setting.get(Number(r))?.setting !== v));
      if (Object.keys(left).length !== Object.keys(draft).length) this.resourceDraft.set(left);
    });
  }

  protected toggleGroup(name: string): void {
    this.open.update((s) => {
      const next = new Set(s);
      if (!next.delete(name)) next.add(name);
      return next;
    });
  }

  protected toggleMin(): void {
    this.store.marketWindow.update((w) => (w === 'min' ? 'open' : 'min'));
  }

  /** Suwak surowca (co 1%): podgląd przy przeciąganiu, nadpisanie po puszczeniu. */
  protected dragResource(resource: number, event: Event): void {
    const value = Number((event.target as HTMLInputElement).value);
    this.resourceDraft.update((d) => ({ ...d, [resource]: value }));
  }

  protected setResource(resource: number, event: Event): void {
    const value = Number((event.target as HTMLInputElement).value);
    this.resourceDraft.update((d) => ({ ...d, [resource]: value }));
    this.session.setTradeResource(resource, value);
  }

  /** Surowiec bez handlu (0%). */
  protected resetResource(resource: number): void {
    this.resourceDraft.update((d) => ({ ...d, [resource]: 0 }));
    this.session.setTradeResource(resource, 0);
  }

  /** Klik w nagłówek kolumny „Ceny”: sortowanie po niej, drugi klik – odwrotnie. */
  protected sortBy(key: SortKey): void {
    this.sort.update((s) => (s.key === key ? { key, dir: s.dir === 1 ? -1 : 1 } : { key, dir: key === 'name' ? 1 : -1 }));
  }

  protected arrow(key: SortKey): string {
    const s = this.sort();
    return s.key === key ? (s.dir === 1 ? ' ▲' : ' ▼') : '';
  }
}
