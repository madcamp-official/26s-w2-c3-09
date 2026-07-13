import { BadRequestException } from '@nestjs/common';
import { idempotencyKeySchema } from '@mousekeeper/contracts';

export function requireIdempotencyKey(value: string | undefined) {
  const parsed = idempotencyKeySchema.safeParse(value);
  if (!parsed.success) {
    throw new BadRequestException({
      code: 'VALIDATION_FAILED',
      message: 'Valid Idempotency-Key is required',
    });
  }
  return parsed.data;
}
