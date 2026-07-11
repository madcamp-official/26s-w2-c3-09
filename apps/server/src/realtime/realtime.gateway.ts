import { Injectable } from '@nestjs/common';
import {
  OnGatewayConnection,
  WebSocketGateway,
  WebSocketServer,
} from '@nestjs/websockets';
import { Server, Socket } from 'socket.io';
import { AuthService } from '../auth/auth.service';

@Injectable()
@WebSocketGateway({
  namespace: '/realtime',
  cors: {
    credentials: true,
    origin: (origin, callback) => {
      const webOrigin = process.env.WEB_ORIGIN;
      if (!origin || (webOrigin && origin === webOrigin)) {
        callback(null, true);
        return;
      }
      callback(new Error('Origin is not allowed'));
    },
  },
})
export class RealtimeGateway implements OnGatewayConnection {
  @WebSocketServer() server!: Server;
  constructor(private readonly auth: AuthService) {}

  async handleConnection(client: Socket) {
    try {
      const raw = client.handshake.auth.token;
      if (typeof raw !== 'string') {
        throw new Error('Missing token');
      }
      const token = raw.startsWith('Bearer ') ? raw.slice(7) : raw;
      const principal = token.startsWith('hm_device_')
        ? await this.auth.authenticateDevice(token.slice(10))
        : await this.auth.authenticate(token);
      await client.join(`user:${principal.userId}`);
      if (principal.deviceId) {
        await client.join(`device:${principal.deviceId}`);
      }
      client.data.principal = principal;
    } catch {
      client.disconnect(true);
    }
  }

  publish(event: {
    eventType: string;
    userId: string;
    deviceId: string | null;
    payload: unknown;
  }) {
    this.server.to(`user:${event.userId}`).emit(event.eventType, event.payload);
  }
}
