import { z } from 'zod';
const schema = z.object({ MOUSEKEEPER_API_URL: z.url(), MOUSEKEEPER_DEVICE_TOKEN: z.string().startsWith('mk_device_'), MOUSEKEEPER_DEVICE_ID: z.uuid() });
export function loadSimulatorConfig(source: NodeJS.ProcessEnv = process.env) {
  const result = schema.safeParse(source);
  if (!result.success) throw new Error(`UNCONFIGURED: ${result.error.issues.map((issue) => issue.path.join('.')).join(', ')}`);
  return result.data;
}
