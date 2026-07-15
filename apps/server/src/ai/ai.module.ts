import { Module } from '@nestjs/common';
import { loadEnvironment } from '../config/environment';
import { AI_PROVIDER, type AiProvider } from './ai.provider';
import { OpenAiResponsesProvider } from './openai-responses.provider';
import { UnconfiguredAiProvider } from './unconfigured-ai.provider';

@Module({
  providers: [{ provide: AI_PROVIDER, useFactory: createAiProvider }],
  exports: [AI_PROVIDER],
})
export class AiModule {}

export function createAiProvider(
  source: NodeJS.ProcessEnv = process.env,
): AiProvider {
  const environment = loadEnvironment(source);
  if (
    environment.AI_PROVIDER === 'openai' &&
    environment.AI_API_KEY &&
    (environment.AI_AGENT_MODEL || environment.AI_MODEL)
  ) {
    return new OpenAiResponsesProvider({
      apiKey: environment.AI_API_KEY,
      model: environment.AI_AGENT_MODEL ?? environment.AI_MODEL!,
      classifierModel: environment.AI_CLASSIFIER_MODEL,
      timeoutMs: environment.AI_TIMEOUT_MS,
      maxOutputTokens: environment.AI_MAX_OUTPUT_TOKENS,
    });
  }
  return new UnconfiguredAiProvider();
}
