import { Module } from '@nestjs/common';
import { AuthModule } from '../auth/auth.module';
import { DatabaseModule } from '../database/database.module';
import { CharacterController } from './character.controller';
import { CharacterService } from './character.service';
@Module({
  imports: [DatabaseModule, AuthModule],
  controllers: [CharacterController],
  providers: [CharacterService],
  exports: [CharacterService],
})
export class CharacterModule {}
