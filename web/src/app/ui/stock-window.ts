import { Component, computed, effect, inject, signal } from '@angular/core';

import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { RESOURCE_GROUPS, RESOURCES } from '../render/resources';
import { amount } from './format';

/** Złoto w interfejsie jest dzielone przez tyle (jak w pasku zasobów). */
const GOLD_UNIT = 10;
/** Złoto – numer surowca (`GOLD` w rdzeniu). */
const GOLD = 30;
/** Zapamiętane ustawienia okna (pozycja, ukrywanie nieposiadanych, zwinięte grupy) – tylko wygoda. */
const PREFS_KEY = 'mapa.stockWindow';
/** Szerokość okna (bez ramki) i wysokość paska tytułu (px, jak w stock-window.css). */
const WIDTH = 320;
const TITLE_HEIGHT = 38;

interface Prefs {
  x: number;
  y: number;
  hideAbsent: boolean;
  collapsed: string[];
}

function loadPrefs(): Prefs | null {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    return raw ? (JSON.parse(raw) as Prefs) : null;
  } catch {
    return null;
  }
}

/**
 * Okno zapasów surowców (otwiera je przycisk ze skrzynką w pasku zasobów): grupy jak legenda mapy
 * zasobów, rozwijane; ilość w puli i wydobycie na dzień; opcja „ukryj nieposiadane”. Okno można
 * przeciągać za pasek tytułu, zminimalizować do paska i zamknąć.
 */
@Component({
  selector: 'app-stock-window',
  templateUrl: './stock-window.html',
  styleUrls: ['./ui.css', './stock-window.css'],
  host: { '(window:resize)': 'onResize()' },
})
export class StockWindow {
  protected readonly store = inject(MapStore);
  private readonly session = inject(GameSession);
  private readonly saved = loadPrefs();
  /** Lewy górny róg okna (px). */
  protected readonly pos = signal({
    x: this.saved?.x ?? Math.max(8, Math.round(innerWidth / 2 - 170)),
    y: this.saved?.y ?? 72,
  });
  protected readonly viewport = signal({ w: innerWidth, h: innerHeight });
  /** Pozycja przycięta do ekranu (zapamiętana mogła pochodzić z szerszego okna, np. przed obrotem telefonu). */
  protected readonly at = computed(() => {
    const { x, y } = this.pos();
    const { w, h } = this.viewport();
    const width = Math.min(WIDTH, w - 16);
    return { x: Math.max(0, Math.min(x, w - width - 2)), y: Math.max(0, Math.min(y, h - TITLE_HEIGHT)) };
  });
  protected readonly hideAbsent = signal(this.saved?.hideAbsent ?? true);
  protected readonly collapsed = signal(new Set(this.saved?.collapsed ?? []));

  protected readonly view = computed(() => {
    const e = this.session.myEconomy();
    if (!e) return null;
    const hide = this.hideAbsent();
    const groups = RESOURCE_GROUPS.map((g) => {
      const items = g.resources
        .filter((r) => r !== GOLD)
        .map((r) => ({
          index: r,
          name: RESOURCES[r].name,
          color: RESOURCES[r].color,
          stock: e.stock[r] ?? 0,
          perDay: e.production[r] ?? 0,
        }))
        .filter((it) => !hide || it.stock > 0 || it.perDay > 0)
        .map((it) => ({ ...it, amount: amount(it.stock), rate: it.perDay > 0 ? `+${amount(it.perDay)}/d` : '' }));
      return { name: g.name, items, open: !this.collapsed().has(g.name) };
    }).filter((g) => g.items.length > 0);
    return {
      groups,
      goldMining: e.goldMining > 0 ? `+${amount(e.goldMining / GOLD_UNIT)}/d` : '',
    };
  });

  constructor() {
    effect(() => {
      const prefs: Prefs = { ...this.pos(), hideAbsent: this.hideAbsent(), collapsed: [...this.collapsed()] };
      try {
        localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
      } catch {
        // Brak dostępu do storage – ustawienia tylko na tę sesję.
      }
    });
  }

  protected onResize(): void {
    this.viewport.set({ w: innerWidth, h: innerHeight });
  }

  protected toggleGroup(name: string): void {
    this.collapsed.update((s) => {
      const next = new Set(s);
      if (!next.delete(name)) next.add(name);
      return next;
    });
  }

  /** Przeciąganie okna za pasek tytułu (mysz i dotyk), w granicach ekranu. */
  protected drag(down: PointerEvent): void {
    if ((down.target as HTMLElement).closest('button')) return;
    const handle = down.currentTarget as HTMLElement;
    handle.setPointerCapture(down.pointerId);
    const start = this.at();
    const move = (e: PointerEvent) => {
      this.pos.set({ x: start.x + e.clientX - down.clientX, y: start.y + e.clientY - down.clientY });
      this.pos.set(this.at());
    };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
  }
}
