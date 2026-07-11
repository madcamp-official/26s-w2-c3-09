import { Inject, Module, OnApplicationShutdown } from '@nestjs/common';
import { createDatabase } from '@housemouse/database';
import { loadEnvironment } from '../config/environment';

export const DATABASE = Symbol('DATABASE');
export const DATABASE_CONNECTION = Symbol('DATABASE_CONNECTION');

@Module({
  providers: [
    {
      provide: DATABASE_CONNECTION,
      useFactory: () => createDatabase(loadEnvironment().DATABASE_URL),
    },
    {
      provide: DATABASE,
      inject: [DATABASE_CONNECTION],
      useFactory: (connection: ReturnType<typeof createDatabase>) =>
        connection.db,
    },
  ],
  exports: [DATABASE],
})
export class DatabaseModule implements OnApplicationShutdown {
  constructor(
    @Inject(DATABASE_CONNECTION)
    private readonly connection: ReturnType<typeof createDatabase>,
  ) {}
  async onApplicationShutdown() {
    await this.connection.close();
  }
}
