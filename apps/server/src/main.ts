import './instrument';
import { NestFactory } from '@nestjs/core';
import {
  FastifyAdapter,
  NestFastifyApplication,
} from '@nestjs/platform-fastify';
import { AppModule } from './app.module';
import { registerRateLimit } from './common/rate-limit';
import { registerRequestLogging } from './common/request-logging';
import { loadEnvironment } from './config/environment';
import { REDIS } from './presence/redis.module';
import Redis from 'ioredis';
import { RedisSocketIoAdapter } from './realtime/redis-socket-io.adapter';

async function bootstrap() {
  const environment = loadEnvironment();
  const app = await NestFactory.create<NestFastifyApplication>(
    AppModule,
    new FastifyAdapter({ trustProxy: environment.NODE_ENV === 'production' }),
  );
  const redis = app.get<Redis>(REDIS);
  registerRequestLogging(app.getHttpAdapter().getInstance());
  await registerRateLimit(
    app.getHttpAdapter().getInstance(),
    redis,
    environment.JWT_OR_DEVICE_TOKEN_SECRET,
  );
  app.useWebSocketAdapter(await RedisSocketIoAdapter.create(app, redis));
  app.enableCors({ origin: environment.WEB_ORIGIN, credentials: true });
  app.enableShutdownHooks();
  await app.listen(environment.PORT, environment.SERVER_HOST);
}
void bootstrap();
