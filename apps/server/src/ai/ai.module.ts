import { Module } from '@nestjs/common';
import { AI_PROVIDER } from './ai.provider';
import { UnconfiguredAiProvider } from './unconfigured-ai.provider';

@Module({
  providers: [{ provide: AI_PROVIDER, useClass: UnconfiguredAiProvider }],
  exports: [AI_PROVIDER],
})
export class AiModule {}
