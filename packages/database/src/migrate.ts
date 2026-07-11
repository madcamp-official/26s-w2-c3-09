import { migrate } from 'drizzle-orm/postgres-js/migrator';
import { createDatabase } from './index';
const url = process.env.DATABASE_URL;
if (!url) throw new Error('UNCONFIGURED: DATABASE_URL');
async function main(databaseUrl: string) {
  const connection = createDatabase(databaseUrl);
  await migrate(connection.db, { migrationsFolder: './migrations' });
  await connection.close();
}
void main(url);
