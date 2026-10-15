import { Component, computed, inject } from '@angular/core';

import { GameSession } from '../game/game-session';

/**
 * Miesiąc gry trwa tyle tur (30 s) – tyle co okres podatku (`economy::TAX_TICKS` w rdzeniu), więc
 * pierwszy dzień miesiąca to zbiór podatków. Dni mają tyle, ile w prawdziwym kalendarzu (28–31),
 * więc dzień trwa ok. 1 s, a rok 6 minut. Na razie tylko na ekranie (rdzeń liczy w turach).
 */
export const TICKS_PER_MONTH = 300;
const MONTHS = ['sty', 'lut', 'mar', 'kwi', 'maj', 'cze', 'lip', 'sie', 'wrz', 'paź', 'lis', 'gru'];
const MONTH_NAMES = [
  'stycznia', 'lutego', 'marca', 'kwietnia', 'maja', 'czerwca',
  'lipca', 'sierpnia', 'września', 'października', 'listopada', 'grudnia',
];
const MONTH_DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/** Data gry po `ticks` turach od początku gry właściwej (1 stycznia roku 0, 00:00). */
export function gameDate(ticks: number): { year: number; month: number; day: number; hour: number } {
  const months = Math.floor(ticks / TICKS_PER_MONTH);
  const month = months % 12;
  // Pora dnia (6 h) w obrębie miesiąca: miesiąc dzieli się równo na dni × 4 pory.
  const quarter = Math.floor(((ticks % TICKS_PER_MONTH) * MONTH_DAYS[month] * 4) / TICKS_PER_MONTH);
  return { year: Math.floor(months / 12), month, day: Math.floor(quarter / 4) + 1, hour: (quarter % 4) * 6 };
}

/** Tur na dzień w danym miesiącu (do przeliczenia przyrostu na dzień). */
export function ticksPerDay(month: number): number {
  return TICKS_PER_MONTH / MONTH_DAYS[month];
}

/** Kalendarz w prawym górnym rogu: dzień, miesiąc, rok i pora dnia (00:00 / 06:00 / 12:00 / 18:00). */
@Component({
  selector: 'app-calendar',
  template: `
    @if (date(); as d) {
      <div class="frame calendar" [class.waiting]="!d.running" [title]="d.title">
        <span class="date">{{ d.day }} {{ d.month }} <small>rok {{ d.year }}</small></span>
        <span class="time">{{ d.hour }}</span>
      </div>
    }
  `,
  styleUrls: ['./ui.css'],
})
export class Calendar {
  private readonly session = inject(GameSession);

  protected readonly date = computed(() => {
    const tick = this.session.tick();
    if (tick === null || !this.session.showsGameMap()) return null;
    const since = this.session.playingSince();
    // Przed grą właściwą (wybór startu) – data startowa, przygaszona.
    const d = gameDate(since === null ? 0 : Math.max(0, tick - since));
    const hour = `${String(d.hour).padStart(2, '0')}:00`;
    return {
      running: since !== null,
      day: d.day,
      month: MONTHS[d.month],
      year: d.year,
      hour,
      title: `${d.day} ${MONTH_NAMES[d.month]} roku ${d.year}, ${hour} – nowy miesiąc to zbiór podatków`,
    };
  });
}
