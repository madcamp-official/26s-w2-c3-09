import { AgentClient } from './client';
import { loadSimulatorConfig } from './config';

async function main() {
  const config = loadSimulatorConfig();
  const client = new AgentClient(config.HOUSEMOUSE_API_URL, config.HOUSEMOUSE_DEVICE_TOKEN, config.HOUSEMOUSE_DEVICE_ID);
  const pending = await client.pending();
  if (pending.length === 0) { process.stdout.write('No pending commands.\n'); return; }
  for (const command of pending) {
    if (command.status === 'QUEUED') await client.update(command.id, 'DELIVERED');
    if (command.status === 'QUEUED' || command.status === 'DELIVERED') await client.update(command.id, 'ANALYZING');
    process.stdout.write(`${JSON.stringify({ commandId: command.id, intent: command.intent, payload: command.payload })}\n`);
  }
  process.stdout.write('Commands remain ANALYZING until the real file engine submits proposals.\n');
}
void main();
