import * as Sentry from '@sentry/nestjs';
import type { ErrorEvent } from '@sentry/nestjs';

const windowsPath = /[A-Za-z]:\\[^\s"']+/g;
const unixPath = /\/(?:home|Users|opt|var|tmp)\/[^\s"']+/g;
const bearerToken = /Bearer\s+[^\s"']+/gi;

function scrubText(value: string | undefined) {
  return value
    ?.replace(windowsPath, '[REDACTED_PATH]')
    .replace(unixPath, '[REDACTED_PATH]')
    .replace(bearerToken, 'Bearer [REDACTED]');
}

export function scrubSentryEvent(event: ErrorEvent): ErrorEvent {
  return {
    ...event,
    message: scrubText(event.message),
    transaction: scrubText(event.transaction),
    request: undefined,
    user: undefined,
    extra: undefined,
    contexts: undefined,
    breadcrumbs: event.breadcrumbs?.map((breadcrumb) => ({
      ...breadcrumb,
      message: scrubText(breadcrumb.message),
      data: undefined,
    })),
    exception: event.exception
      ? {
          ...event.exception,
          values: event.exception.values?.map((exception) => ({
            ...exception,
            value: scrubText(exception.value),
            stacktrace: exception.stacktrace
              ? {
                  ...exception.stacktrace,
                  frames: exception.stacktrace.frames?.map((frame) => ({
                    ...frame,
                    abs_path: undefined,
                    vars: undefined,
                  })),
                }
              : undefined,
          })),
        }
      : undefined,
  };
}

export function initializeSentry(source: NodeJS.ProcessEnv = process.env) {
  const dsn = source.SENTRY_DSN?.trim();
  if (!dsn) return false;
  try {
    const parsed = new URL(dsn);
    if (!['https:', 'http:'].includes(parsed.protocol)) throw new Error();
  } catch {
    throw new Error('UNCONFIGURED: SENTRY_DSN');
  }
  Sentry.init({
    dsn,
    sendDefaultPii: false,
    tracesSampleRate: 0,
    enableLogs: false,
    beforeSend: scrubSentryEvent,
  });
  return true;
}
