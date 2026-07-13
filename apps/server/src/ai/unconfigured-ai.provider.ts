import type {
  AiProvider,
  AiProviderResult,
  AiUnavailableResult,
  ChatContext,
  RuleDraftResult,
  RuleTranslationContext,
} from './ai.provider';

export class UnconfiguredAiProvider implements AiProvider {
  async classifyAndRespond(_input: ChatContext): Promise<AiProviderResult> {
    return this.unconfigured();
  }

  async translateRule(
    _input: RuleTranslationContext,
  ): Promise<RuleDraftResult> {
    return this.unconfigured();
  }

  private unconfigured(): AiUnavailableResult {
    return {
      status: 'UNCONFIGURED',
      code: 'AI_PROVIDER_UNCONFIGURED',
    };
  }
}
