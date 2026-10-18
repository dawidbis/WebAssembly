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
/** Rezerwa i nasycenie w dniach potrzeby (`market::RESERVE_DAYS`, `SATURATION_DAYS`). */
const RESERVE_DAYS = 10;
const SATURATION_DAYS = 30;
/** Nadpisanie bilansu surowca – wartości w liście (null – jak grupa). */
const OVERRIDES: readonly { value: number | null; label: string }[] = [
  { value: null, label: 'jak grupa' },
  { value: -100, label: 'import 100%' },
  { value: -50, label: 'import 50%' },
  { value: 0, label: 'bez handlu' },
  { value: 50, label: 'eksport 50%' },
  { value: 100, label: 'eksport 100%' },
];

type Tab = 'balance' | 'prices' | 'deals';

interface Prefs {
  tab: Tab;
  open: string[];
}

function loadPrefs(): Partial<Prefs> {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    return raw ? (JSON.parse(raw) as Partial<Prefs>) : {};
  } catch {
    return {};
  }
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
 * Dlaczego automat handluje surowcem (albo nie): potrzeba fabryk, zapas w dniach, rezerwa i nasycenie –
 * automat nie może być czarną skrzynką.
 */
function reason(r: MarketResourceReport): string {
  const days = r.need > 0 ? r.stock / r.need : Infinity;
  const name = r.name;
  if (r.setting < 0) {
    if (r.need <= 0) return 'Fabryki nie potrzebują – import nic nie kupi';
    if (r.stock >= r.saturation) return `Zapas ${name} na ${Math.floor(days)} dni – nasycony (import do ${SATURATION_DAYS} dni potrzeby)`;
    return `Brak ${name} do ${SATURATION_DAYS} dni potrzeby fabryk (zapas na ${Math.floor(days)} dni) – import ${Math.abs(r.setting)}% brakującej ilości`;
  }
  if (r.setting > 0) {
    if (r.stock <= r.reserve) return r.need > 0 ? `Zapas tylko na ${Math.floor(days)} dni – poniżej rezerwy (${RESERVE_DAYS} dni) nic nie sprzedaje` : 'Brak zapasu do sprzedaży';
    return `Nadwyżka ponad rezerwę ${RESERVE_DAYS} dni potrzeby – eksport ${r.setting}%`;
  }
  return r.need > 0 ? `Fabryki potrzebują ${amount(r.need)}/d, zapas na ${Number.isFinite(days) ? Math.floor(days) : '∞'} dni` : '';
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
  /** Suwak grupy w trakcie przeciągania (zanim wróci raport z nowym ustawieniem). */
  protected readonly draft = signal<Record<number, number>>({});
  protected readonly initial = { x: Math.max(8, Math.round(innerWidth / 2 - 230)), y: 96 };
  protected readonly overrides = OVERRIDES;
  protected readonly balanceText = balanceText;

  protected readonly view = computed(() => {
    const m = this.session.market();
    if (!m) return null;
    const byId = new Map(m.resources.map((r) => [r.resource, r]));
    const draft = this.draft();
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
        // Bez potrzeby i bez zapasu nie ma czego tłumaczyć.
        reason: r.need > 0 || r.stock > 0 ? reason(r) : '',
        balance: balanceText(r.setting),
      };
    };
    return {
      groups: m.groups.map((g, index) => ({
        index,
        name: g.name,
        setting: draft[index] ?? g.setting,
        open: this.open().has(g.name),
        items: g.resources.map((id) => byId.get(id)).filter((r) => r !== undefined).map(row),
      })),
      prices: m.resources.map(row),
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
      share: `${(m.share * 100).toFixed(1).replace('.', ',')}%`,
      trader: `${Math.round(m.traderDelivery * 100)}%`,
    };
  });

  constructor() {
    effect(() => {
      const prefs: Prefs = { tab: this.tab(), open: [...this.open()] };
      try {
        localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
      } catch {
        // Brak dostępu do storage – ustawienia tylko na tę sesję.
      }
    });
    // Raport z nowym ustawieniem grupy – szkic suwaka już niepotrzebny.
    effect(() => {
      const m = this.session.market();
      const draft = this.draft();
      if (!m || Object.keys(draft).length === 0) return;
      const left = Object.fromEntries(Object.entries(draft).filter(([g, v]) => m.groups[Number(g)]?.setting !== v));
      if (Object.keys(left).length !== Object.keys(draft).length) this.draft.set(left);
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

  /** Suwak grupy: podgląd przy przeciąganiu, intencja po puszczeniu. */
  protected dragGroup(group: number, event: Event): void {
    const value = Number((event.target as HTMLInputElement).value);
    this.draft.update((d) => ({ ...d, [group]: value }));
  }

  protected setGroup(group: number, event: Event): void {
    const value = Number((event.target as HTMLInputElement).value);
    this.draft.update((d) => ({ ...d, [group]: value }));
    this.session.setTradeGroup(group, value);
  }

  protected setResource(resource: number, event: Event): void {
    const raw = (event.target as HTMLSelectElement).value;
    this.session.setTradeResource(resource, raw === '' ? null : Number(raw));
  }
}
