import { Component, computed, effect, inject, signal } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { RESOURCE_GROUPS, RESOURCES } from '../render/resources';
import { FloatingWindow } from './floating-window';
import { amount } from './format';
import { GoodsIcon, SUPPLY_ICONS } from './goods-icon';
import { Recipe } from './recipe';
import { COMMAND_UNLOCKS, UNIVERSITY_UNLOCKS } from './unlocks';
import type { Supply } from '../../generated/Supply';

/** Złoto w interfejsie jest dzielone przez tyle (jak w pasku zasobów). */
const GOLD_UNIT = 10;
/** Złoto – numer surowca (`GOLD` w rdzeniu). */
const GOLD = 30;
/** Rodzaje zaopatrzenia (kolejność `goods::Supply` w rdzeniu). */
const SUPPLIES = Object.keys(SUPPLY_ICONS) as Supply[];
/** Grupa zaopatrzenia w oknie (zwijana jak grupy surowców). */
const SUPPLY_GROUP = 'Zaopatrzenie';
/** Zapamiętane ustawienia okna (ukrywanie nieposiadanych, zwinięte grupy, zakładka) – tylko wygoda. */
const PREFS_KEY = 'mapa.stockWindow';

type Tab = 'stock' | 'recipes' | 'unlocks';

interface Prefs {
  hideAbsent: boolean;
  collapsed: string[];
  tab: Tab;
}

/** Zmiana na dzień: „+1,1/d”, „−0,4/d”, przy zerze pusto. */
function rate(perDay: number): string {
  if (Math.abs(perDay) < 0.05) return '';
  return `${perDay > 0 ? '+' : '−'}${amount(Math.abs(perDay))}/d`;
}

function loadPrefs(): Partial<Prefs> {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    return raw ? (JSON.parse(raw) as Partial<Prefs>) : {};
  } catch {
    return {};
  }
}

/**
 * Okno zapasów (przycisk ze skrzynką w pasku zasobów; przeciągane, minimalizacja, zamykanie). Zakładki:
 * zapasy (grupy jak legenda mapy zasobów, rozwijane; ilość i bilans na dzień; „ukryj nieposiadane”),
 * receptury (drzewko zaopatrzenia: składniki z ikonami, od którego poziomu centrum dowodzenia)
 * i odblokowania (centrum dowodzenia, uniwersytety).
 */
@Component({
  selector: 'app-stock-window',
  imports: [FloatingWindow, GoodsIcon, Recipe],
  templateUrl: './stock-window.html',
  styleUrls: ['./ui.css', './stock-window.css'],
})
export class StockWindow {
  protected readonly store = inject(MapStore);
  protected readonly session = inject(GameSession);
  private readonly saved = loadPrefs();
  protected readonly hideAbsent = signal(this.saved.hideAbsent ?? true);
  protected readonly collapsed = signal(new Set(this.saved.collapsed ?? []));
  protected readonly tab = signal<Tab>(this.saved.tab ?? 'stock');
  protected readonly initial = { x: Math.max(8, Math.round(innerWidth / 2 - 180)), y: 72 };
  protected readonly commandUnlocks = COMMAND_UNLOCKS;
  protected readonly universityUnlocks = UNIVERSITY_UNLOCKS;

  protected readonly view = computed(() => {
    const e = this.session.myEconomy();
    if (!e) return null;
    const hide = this.hideAbsent();
    const groups = RESOURCE_GROUPS.map((g) => {
      const items = g.resources
        .filter((r) => r !== GOLD)
        .map((r) => ({
          index: r,
          supply: null as Supply | null,
          name: RESOURCES[r].name,
          stock: e.stock[r] ?? 0,
          // Bilans na dzień: wydobycie minus zużycie (fabryki, nawożenie, konie w atakach).
          perDay: (e.production[r] ?? 0) - (e.resourceUse[r] ?? 0),
          used: (e.resourceUse[r] ?? 0) > 0,
        }))
        .filter((it) => !hide || it.stock > 0 || it.perDay !== 0 || it.used)
        .map((it) => ({ ...it, amount: amount(it.stock), rate: rate(it.perDay), down: it.perDay < 0 }));
      return { name: g.name, items, open: !this.collapsed().has(g.name) };
    }).filter((g) => g.items.length > 0);
    // Zaopatrzenie: zapas i bilans na dzień (produkcja fabryk − zużycie).
    const supplies = SUPPLIES.map((s, index) => ({
      index: 100 + index,
      supply: s as Supply | null,
      name: SUPPLY_ICONS[s].name,
      stock: e.supplies[index] ?? 0,
      perDay: (e.supplyProduction[index] ?? 0) - (e.supplyUse[index] ?? 0),
      used: (e.supplyUse[index] ?? 0) > 0,
    }))
      // Zaopatrzenia jest 7 rodzajów – zawsze widoczne (także przy zerowym zapasie, np. paliwo w walce).
      .map((it) => ({ ...it, amount: amount(it.stock), rate: rate(it.perDay), down: it.perDay < 0 }));
    if (supplies.length > 0) groups.unshift({ name: SUPPLY_GROUP, items: supplies, open: !this.collapsed().has(SUPPLY_GROUP) });
    return {
      groups,
      goldMining: e.goldMining > 0 ? `+${amount(e.goldMining / GOLD_UNIT)}/d` : '',
      command: e.commandLevel,
      universities: e.universities,
    };
  });

  constructor() {
    effect(() => {
      const prefs: Prefs = { hideAbsent: this.hideAbsent(), collapsed: [...this.collapsed()], tab: this.tab() };
      try {
        localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
      } catch {
        // Brak dostępu do storage – ustawienia tylko na tę sesję.
      }
    });
  }

  protected toggleGroup(name: string): void {
    this.collapsed.update((s) => {
      const next = new Set(s);
      if (!next.delete(name)) next.add(name);
      return next;
    });
  }

  protected toggleMin(): void {
    this.store.stockWindow.update((w) => (w === 'min' ? 'open' : 'min'));
  }
}
