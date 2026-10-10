import { Component, inject } from '@angular/core';

import { Lobby } from '../game/lobby';

/** Najwyższy seed (u32). */
const MAX_SEED = 4294967295;

/**
 * Lobby gracza: nazwa, lista pokoi (gracze online z heartbeatu serwera), założenie pokoju.
 * W trakcie gry zwinięte do przycisku z nazwą pokoju. Bez API lobby (tryb lokalny) – niewidoczne.
 */
@Component({
  selector: 'app-lobby-panel',
  template: `
    @if (lobby.available()) {
      @if (lobby.open()) {
        <section class="frame lobby" aria-labelledby="lobby-title">
          <header>
            <h2 id="lobby-title">Pokoje</h2>
            @if (lobby.current()) {
              <button type="button" (click)="lobby.hide()">Wróć do gry</button>
            }
          </header>

          <label class="field">
            <span>Twoja nazwa</span>
            <input type="text" maxlength="24" [value]="lobby.playerName()" (change)="setName($event)" />
          </label>

          <ul class="rooms">
            @for (room of lobby.rooms(); track room.id) {
              <li [class.here]="room.id === lobby.current()?.id">
                <span class="room-name">{{ room.name }}</span>
                <span class="room-meta">{{ room.players }}/{{ room.maxPlayers }} · seed {{ room.seed }}</span>
                <button
                  type="button"
                  [disabled]="lobby.busy() || room.players >= room.maxPlayers"
                  (click)="lobby.join(room)"
                >
                  {{ room.id === lobby.current()?.id ? 'Wróć' : 'Dołącz' }}
                </button>
              </li>
            } @empty {
              <li class="empty">Brak otwartych pokoi – załóż pierwszy.</li>
            }
          </ul>

          <form class="create" (submit)="create($event)">
            <input name="room" type="text" maxlength="24" placeholder="Nazwa nowego pokoju" required />
            <input name="seed" type="number" min="0" [max]="maxSeed" placeholder="Seed" title="Seed mapy (puste = losowy)" />
            <button type="submit" [disabled]="lobby.busy()">Załóż i dołącz</button>
          </form>

          @if (lobby.error(); as error) {
            <p class="error" role="alert">{{ error }}</p>
          }
        </section>
      } @else {
        <button type="button" class="frame room-chip" (click)="lobby.show()" title="Lista pokoi">
          Pokój: <strong>{{ lobby.current()?.name ?? '—' }}</strong> · zmień
        </button>
      }
    }
  `,
  styleUrls: ['./ui.css', './lobby-panel.css'],
})
export class LobbyPanel {
  protected readonly lobby = inject(Lobby);
  protected readonly maxSeed = MAX_SEED;

  protected setName(event: Event): void {
    const name = (event.target as HTMLInputElement).value.trim();
    if (name) this.lobby.setName(name);
  }

  protected create(event: SubmitEvent): void {
    event.preventDefault();
    const form = event.target as HTMLFormElement;
    const data = new FormData(form);
    const name = String(data.get('room') ?? '').trim();
    const seedText = String(data.get('seed') ?? '').trim();
    const seed = seedText === '' ? null : Number(seedText);
    if (!name || (seed !== null && (!Number.isInteger(seed) || seed < 0 || seed > MAX_SEED))) return;
    void this.lobby.create(name, seed).then(() => form.reset());
  }
}
