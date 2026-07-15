/**
 * Manual live-API check for the AI prompt (mouse persona + intent classification).
 *
 * This is NOT part of CI. It only runs when AI_API_KEY and AI_MODEL are present in the
 * environment, so a normal `jest` run simply skips it. Run it yourself with your own key:
 *
 *   AI_API_KEY=sk-... AI_MODEL=gpt-4.1-mini npx jest live-check.manual --runInBand
 *
 * (On PowerShell: `$env:AI_API_KEY='sk-...'; $env:AI_MODEL='gpt-4.1-mini'; npx jest live-check.manual --runInBand`)
 *
 * It prints the classified kind + reply for a handful of sample prompts covering each
 * command kind, so you can eyeball whether classification is right and the reply reads
 * like a squeaky little mouse.
 */
import { OpenAiResponsesProvider } from './openai-responses.provider';

const apiKey = process.env.AI_API_KEY ?? '';
const model = process.env.AI_MODEL ?? '';
const enabled = apiKey.trim().length > 0 && model.trim().length > 0;

const SAMPLES: Array<{ label: string; message: string; expectKind: string }> = [
  { label: '인사/잡담', message: '안녕! 오늘 뭐 할 수 있어?', expectKind: 'NO_ACTION' },
  { label: '조회(QUERY)', message: '다운로드 폴더에 pdf 파일 뭐 있는지 보여줘', expectKind: 'QUERY' },
  { label: '이름변경(COMMAND)', message: '스크린샷들 이름을 날짜순으로 바꿔줘', expectKind: 'COMMAND_DRAFT' },
  { label: '정리제안(ANALYZE)', message: '내 폴더 좀 정리해줘', expectKind: 'COMMAND_DRAFT' },
  { label: '규칙(RULE)', message: '앞으로 png는 항상 이미지 폴더로 옮겨줘', expectKind: 'RULE_DRAFT' },
];

const maybe = enabled ? describe : describe.skip;

maybe('AI live check (manual)', () => {
  const provider = new OpenAiResponsesProvider({ apiKey, model });

  for (const sample of SAMPLES) {
    it(`${sample.label} => ${sample.expectKind}`, async () => {
      const result = await provider.classifyAndRespond({
        userId: 'live-check-user',
        roomId: 'live-check-room',
        sessionId: 'live-check-session',
        sourceMessage: { id: 'msg-1', content: sample.message },
      });

      const reply =
        'reply' in result
          ? result.reply
          : 'responseSummary' in result
            ? result.responseSummary
            : 'confirmationSummary' in result
              ? result.confirmationSummary
              : '';

      // eslint-disable-next-line no-console
      console.log(
        `\n[${sample.label}]\n  message : ${sample.message}\n  status  : ${result.status}\n  kind    : ${'kind' in result ? result.kind : '-'} (expected ${sample.expectKind})\n  reply   : ${reply}`,
      );

      expect(result.status).toBe('READY');
    }, 30_000);
  }
});

if (!enabled) {
  // Keeps jest from erroring on an empty file when the check is skipped.
  it('AI live check skipped (set AI_API_KEY and AI_MODEL to enable)', () => {
    expect(enabled).toBe(false);
  });
}
