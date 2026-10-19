import { Component, computed, effect, input, output, signal } from '@angular/core';

/** Wysokość paska tytułu (px) – okno nie ucieka pod krawędź ekranu dalej niż do niego. */
const TITLE_HEIGHT = 38;

function loadPos(key: string): { x: number; y: number } | null {
  try {
    const raw = localStorage.getItem(key);
    const p = raw ? (JSON.parse(raw) as { x: number; y: number }) : null;
    return p && Number.isFinite(p.x) && Number.isFinite(p.y) ? p : null;
  } catch {
    return null;
  }
}

/**
 * Okno nad mapą przeciągane za pasek tytułu (mysz i dotyk, w granicach ekranu), z zamykaniem
 * i opcjonalną minimalizacją. Pozycja zapamiętana pod `key` w localStorage (tylko wygoda).
 * Treść – `<ng-content>`, dodatki w pasku tytułu – `[title-extra]`.
 */
@Component({
  selector: 'app-floating-window',
  templateUrl: './floating-window.html',
  styleUrls: ['./ui.css', './floating-window.css'],
  host: { '(window:resize)': 'onResize()' },
})
export class FloatingWindow {
  readonly title = input.required<string>();
  /** Klucz pozycji w localStorage. */
  readonly key = input.required<string>();
  /** Szerokość (px) i domyślna pozycja (bez zapamiętanej). */
  readonly width = input(320);
  readonly initial = input<{ x: number; y: number }>({ x: 80, y: 72 });
  readonly minimizable = input(false);
  readonly minimized = input(false);
  /** Zadokowane w lewym dolnym rogu (pozycja stała, bez przeciągania). */
  readonly docked = input(false);
  readonly closed = output<void>();
  readonly toggleMin = output<void>();

  protected readonly pos = signal<{ x: number; y: number } | null>(null);
  protected readonly viewport = signal({ w: innerWidth, h: innerHeight });
  /** Pozycja przycięta do ekranu (zapamiętana mogła pochodzić z szerszego okna). */
  protected readonly at = computed(() => {
    const { x, y } = this.pos() ?? this.initial();
    const { w, h } = this.viewport();
    const width = Math.min(this.width(), w - 16);
    return { x: Math.max(0, Math.min(x, w - width - 2)), y: Math.max(0, Math.min(y, h - TITLE_HEIGHT)) };
  });

  constructor() {
    effect(() => {
      if (this.pos() === null) this.pos.set(loadPos(this.key()));
    });
  }

  protected onResize(): void {
    this.viewport.set({ w: innerWidth, h: innerHeight });
  }

  /** Przeciąganie za pasek tytułu (przyciski w pasku nie przeciągają). */
  protected drag(down: PointerEvent): void {
    if (this.docked()) return;
    if ((down.target as HTMLElement).closest('button, select, input')) return;
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
      try {
        localStorage.setItem(this.key(), JSON.stringify(this.at()));
      } catch {
        // Bez storage – pozycja tylko na tę sesję.
      }
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
  }
}
