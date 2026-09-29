import { Injectable, signal } from '@angular/core';

import type { ClientMsg } from '../../generated/ClientMsg';
import type { ServerMsg } from '../../generated/ServerMsg';

export type ConnectionStatus = 'connecting' | 'online' | 'offline';

/** Połączenie WebSocket z serwerem-przekaźnikiem tur. */
@Injectable({ providedIn: 'root' })
export class Transport {
  private socket?: WebSocket;
  private retry?: ReturnType<typeof setTimeout>;

  readonly status = signal<ConnectionStatus>('offline');
  readonly playerId = signal<number | null>(null);
  readonly lastTick = signal<number | null>(null);

  connect(): void {
    clearTimeout(this.retry);
    const protocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${protocol}://${location.host}/ws`);
    this.socket = socket;
    this.status.set('connecting');

    socket.onopen = () => {
      this.status.set('online');
      this.send({ type: 'join', name: 'dev' });
    };
    socket.onmessage = ({ data }) => this.onMessage(JSON.parse(data as string) as ServerMsg);
    socket.onclose = () => {
      this.status.set('offline');
      this.playerId.set(null);
      this.retry = setTimeout(() => this.connect(), 5000);
    };
  }

  send(msg: ClientMsg): void {
    if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify(msg));
  }

  private onMessage(msg: ServerMsg): void {
    switch (msg.type) {
      case 'welcome':
        this.playerId.set(msg.player);
        break;
      case 'turn':
        // TODO (mechaniki): przekazać turę do workera → WasmGame.applyTurn, odesłać hash.
        this.lastTick.set(msg.turn.tick);
        break;
      case 'desync':
        console.warn(`Desync na ticku ${msg.tick}`);
        break;
    }
  }
}
