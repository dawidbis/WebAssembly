import { Component, computed, effect, inject, signal } from '@angular/core';

import { GameSession } from '../game/game-session';
import { Transport } from '../game/transport';

/** Jak długo widać wiadomość o cudzej aneksji (ms). */
const TOAST_MS = 9000;

/**
 * Komunikaty pod górnym paskiem: problemy z grą (rozbieżność stanu, błąd, brak połączenia) oraz koniec gry
 * – przegrana (stolica zdobyta, państwo przyłączone) albo zwycięstwo (wszyscy inni przyłączeni) – i na chwilę
 * wiadomość o każdej aneksji. Bez serwera od początku (tryb lokalny, np. strojenie generatora) nic nie pokazuje.
 */
@Component({
  selector: 'app-game-status',
  template: `
    @if (notice(); as notice) {
      <p class="frame notice" role="alert">{{ notice }}</p>
    }
    @if (outcome(); as o) {
      <p class="frame notice outcome" [class.won]="o.won" role="status">{{ o.text }}</p>
    }
    @for (t of toasts(); track t.key) {
      <p class="frame notice toast" role="status">{{ t.text }}</p>
    }
  `,
  styleUrl: './ui.css',
})
export class GameStatus {
  private readonly session = inject(GameSession);
  private readonly transport = inject(Transport);
  /** Aneksje już pokazane (klucz: gracz pokonany). */
  private readonly seen = new Set<number>();
  protected readonly toasts = signal<{ key: number; text: string }[]>([]);

  constructor() {
    effect(() => {
      const economy = this.session.economy();
      const me = this.session.player();
      for (const r of economy) {
        if (r.annexedBy == null || this.seen.has(r.player)) continue;
        this.seen.add(r.player);
        // Własna porażka i zwycięstwo mają stały komunikat (`outcome`).
        if (r.player === me || r.annexedBy === me) continue;
        const text = `${this.session.playerName(r.annexedBy)} zdobył stolicę gracza ${this.session.playerName(r.player)} i przyłączył całe jego państwo`;
        this.toasts.update((t) => [...t, { key: r.player, text }]);
        setTimeout(() => this.toasts.update((t) => t.filter((x) => x.key !== r.player)), TOAST_MS);
      }
    });
  }

  protected readonly notice = computed(() => {
    const error = this.session.error();
    if (error) return `Gra zatrzymana: ${error}`;
    const desync = this.session.desync();
    if (desync !== null) return `Stan gry rozjechał się z innymi graczami (tura ${desync}) – odśwież stronę`;
    if (this.session.config() && this.transport.status() !== 'online') return 'Brak połączenia z serwerem – ponawiam…';
    return null;
  });

  /** Koniec gry dla tego gracza: przegrana albo zwycięstwo (null – gra trwa). */
  protected readonly outcome = computed(() => {
    const economy = this.session.economy();
    const nation = this.session.myNation();
    if (!nation || economy.length < 2) return null;
    const mine = economy[nation - 1];
    if (!mine) return null;
    if (mine.annexedBy != null) {
      return { won: false, text: `Twoja stolica padła – ${this.session.playerName(mine.annexedBy)} przyłączył całe państwo. Możesz dalej oglądać grę.` };
    }
    if (economy.every((r, k) => k === nation - 1 || r.annexedBy != null)) {
      return { won: true, text: 'Zwycięstwo! Wszystkie inne państwa zostały przyłączone.' };
    }
    return null;
  });
}
