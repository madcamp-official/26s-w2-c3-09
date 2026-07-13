import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { CharacterModule } from '../character/character.module';
import { DatabaseModule } from '../database/database.module';
import { RedisModule } from '../presence/redis.module';
import { HomeController } from './home.controller';
import { HomeService } from './home.service';

@Module({
  imports: [AuthModule, CharacterModule, DatabaseModule, RedisModule],
  controllers: [HomeController],
  providers: [HomeService],
})
export class HomeModule {}
