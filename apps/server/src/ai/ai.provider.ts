export const AI_PROVIDER = Symbol('AI_PROVIDER');

export type ChatContext = {
  userId: string;
  roomId: string;
  sessionId: string;
  sourceMessage: {
    id: string;
    content: string;
  };
};

export type RuleTranslationContext = {
  userId: string;
  roomId: string;
  instruction: string;
};

export type AiUnavailableResult = {
  status: 'UNCONFIGURED';
  code: 'AI_PROVIDER_UNCONFIGURED';
};

export type AiProviderResult = AiUnavailableResult;

export interface AiProvider {
  classifyAndRespond(input: ChatContext): Promise<AiProviderResult>;
  translateRule(input: RuleTranslationContext): Promise<AiProviderResult>;
}
