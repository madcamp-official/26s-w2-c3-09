import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { CharacterController } from './character.controller';
@Module({
  imports: [DatabaseModule, AuthModule],
  controllers: [CharacterController],
})
export class CharacterModule {}
