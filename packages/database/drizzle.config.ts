import { defineConfig } from 'drizzle-kit';
const url = process.env.DATABASE_URL;
if (!url) throw new Error('UNCONFIGURED: DATABASE_URL');
export default defineConfig({ dialect: 'postgresql', schema: './src/schema.ts', out: './migrations', dbCredentials: { url } });
