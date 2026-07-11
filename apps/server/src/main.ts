import { NestFactory } from '@nestjs/core';
import { FastifyAdapter, NestFastifyApplication } from '@nestjs/platform-fastify';
import { AppModule } from './app.module';
import { loadEnvironment } from './config/environment';

async function bootstrap() {
  const environment = loadEnvironment();
  const app = await NestFactory.create<NestFastifyApplication>(AppModule, new FastifyAdapter());
  app.enableShutdownHooks();
  await app.listen(environment.PORT, '0.0.0.0');
}
void bootstrap();
