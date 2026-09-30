import { Injectable, computed, inject, signal } from '@angular/core';

import type { GameConfig } from '../../generated/GameConfig';
import type { ServerMsg } from '../../generated/ServerMsg';
import { sameConfig, sameParams, type GameEvent, type TickHash } from '../worker/protocol';
import { MapStore } from './map-store';
import { Transport } from './transport';
import { WorkerBridge } from './worker-bridge';

/** Po tylu ms bez `Welcome` klient generuje mapę z domyślnych parametrów (serwer nie odpowiada). */
const OFFLINE_FALLBACK_MS = 3000;

/**
 * Pętla lockstep po stronie klienta: `Welcome` → mapa z `GameConfig` (na ekran i do gry, generowana
 * raz) → worker buduje z niej `WasmGame`, wykonuje tury z serwera i zwraca hashe stanu, które
 * sesja odsyła serwerowi. Bez serwera mapa powstaje z domyślnych parametrów, a gry nie ma.
 */
@Injectable({ providedIn: 'root' })
export class GameSession {
  private readonly transport = inject(Transport);
  private readonly bridge = inject(WorkerBridge);
  private readonly store = inject(MapStore);

  /** Konfiguracja gry z ostatniego `Welcome` (null = jeszcze bez serwera). */
  readonly config = signal<GameConfig | null>(null);
  readonly player = signal<number | null>(null);
  /** Ostatnia tura rozesłana przez serwer. */
  readonly serverTick = signal<number | null>(null);
  /** Tury wykonane przez grę w workerze (null = gra jeszcze nie ruszyła, np. mapa się generuje). */
  readonly tick = signal<number | null>(null);
  /** Bieżący hash stanu gry. */
  readonly hash = signal<number | null>(null);
  /** Ostatni hash odesłany serwerowi. */
  readonly sentHash = signal<TickHash | null>(null);
  /** Pierwszy tick, na którym serwer zgłosił rozbieżność stanu (null = brak). */
  readonly desync = signal<number | null>(null);
  /** Gra stanęła z błędem (np. inna wersja generatora niż na serwerze). */
  readonly error = signal<string | null>(null);
  /** Czy na ekranie jest mapa gry (a nie lokalny podgląd z panelu debugu). */
  readonly showsGameMap = computed(() => {
    const config = this.config();
    const map = this.store.map();
    return !!config && !!map && sameParams(map.params, config.map);
  });

  start(): void {
    this.transport.onMessage = (msg) => this.onServer(msg);
    this.transport.onClose = () => this.onClose();
    this.bridge.onGame = (event) => this.onWorker(event);
    this.transport.connect();
    void this.store.init();
    // Mapę generujemy dopiero z konfiguracji serwera. Gdy serwer milczy – z domyślnych parametrów.
    setTimeout(() => {
      if (!this.config()) void this.store.showDefault();
    }, OFFLINE_FALLBACK_MS);
  }

  private onServer(msg: ServerMsg): void {
    switch (msg.type) {
      case 'welcome': {
        const previous = this.config();
        this.config.set(msg.config);
        this.player.set(msg.player);
        this.serverTick.set(null);
        this.tick.set(null);
        this.hash.set(null);
        this.sentHash.set(null);
        this.desync.set(null);
        this.error.set(null);
        // Gra najpierw: tury, które zaraz przyjdą, muszą trafić do workera po `startGame`.
        this.bridge.startGame(msg.config, msg.catchup);
        // Ta sama gra po ponownym połączeniu – mapa na ekranie zostaje (także lokalny podgląd z panelu).
        if (!previous || !sameConfig(previous, msg.config)) this.store.show(msg.config.map);
        break;
      }
      case 'turn':
        this.serverTick.set(msg.turn.tick);
        this.bridge.turn(msg.turn);
        break;
      case 'desync':
        // Serwer zgłasza każdy rozbieżny hash – wystarczy pierwszy (reszta jest w logu serwera).
        if (this.desync() === null) {
          console.warn(`Desync na ticku ${msg.tick}`);
          this.desync.set(msg.tick);
        }
        break;
    }
  }

  private onWorker(event: GameEvent): void {
    if (event.type === 'gameError') {
      console.error(`Gra zatrzymana: ${event.message}`);
      this.error.set(event.message);
      return;
    }
    this.tick.set(event.tick);
    this.hash.set(event.hash);
    for (const { tick, hash } of event.hashes) this.transport.send({ type: 'hash', tick, hash });
    const last = event.hashes.at(-1);
    if (last) this.sentHash.set(last);
  }

  private onClose(): void {
    this.player.set(null);
    // Bez serwera od początku – mapa z domyślnych parametrów (nie czekamy na limit czasu).
    if (!this.config()) void this.store.showDefault();
  }
}
