import { commandStatusSchema } from '@mousekeeper/contracts';
import { z } from 'zod';

const commandSchema = z.object({ id: z.uuid(), intent: z.string(), payload: z.record(z.string(), z.unknown()), status: commandStatusSchema });
export type PendingCommand = z.infer<typeof commandSchema>;

export class AgentClient {
  constructor(private readonly baseUrl: string, private readonly token: string, private readonly deviceId: string) {}
  private async request(path: string, init?: RequestInit) {
    const response = await fetch(new URL(path, this.baseUrl), { ...init, headers: { authorization: `Bearer ${this.token}`, 'content-type': 'application/json', ...init?.headers } });
    if (!response.ok) throw new Error(`API request failed: ${response.status} ${await response.text()}`);
    return response.json() as Promise<unknown>;
  }
  async pending() {
    return z.array(commandSchema).parse(await this.request(`/v1/devices/${this.deviceId}/commands/pending`));
  }
  async update(commandId: string, status: 'DELIVERED' | 'ANALYZING') {
    return this.request(`/v1/devices/${this.deviceId}/commands/${commandId}/status`, { method: 'PATCH', body: JSON.stringify({ status }) });
  }
}
