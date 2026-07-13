import type {
  AiProvider,
  AiProviderResult,
  ChatContext,
  RuleTranslationContext,
} from './ai.provider';

export class UnconfiguredAiProvider implements AiProvider {
  async classifyAndRespond(_input: ChatContext): Promise<AiProviderResult> {
    return this.unconfigured();
  }

  async translateRule(
    _input: RuleTranslationContext,
  ): Promise<AiProviderResult> {
    return this.unconfigured();
  }

  private unconfigured(): AiProviderResult {
    return {
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    };
  }
}
