import 'package:sentry_flutter/sentry_flutter.dart';

final _windowsPath = RegExp(r'''[A-Za-z]:\\[^\s"']+''');
final _unixPath = RegExp(r'''\/(?:home|Users|opt|var|tmp)\/[^\s"']+''');
final _bearerToken = RegExp(r'''Bearer\s+[^\s"']+''', caseSensitive: false);

String scrubSentryText(String value) => value
    .replaceAll(_windowsPath, '[REDACTED_PATH]')
    .replaceAll(_unixPath, '[REDACTED_PATH]')
    .replaceAll(_bearerToken, 'Bearer [REDACTED]');

SentryEvent scrubMobileSentryEvent(SentryEvent event, Hint _) => SentryEvent(
  eventId: event.eventId,
  timestamp: event.timestamp,
  platform: event.platform,
  release: event.release,
  dist: event.dist,
  environment: event.environment,
  level: event.level,
  fingerprint: event.fingerprint,
  transaction: event.transaction == null
      ? null
      : scrubSentryText(event.transaction!),
  message: event.message == null
      ? null
      : SentryMessage(scrubSentryText(event.message!.formatted)),
  exceptions: event.exceptions
      ?.map(
        (exception) => SentryException(
          type: exception.type,
          value: exception.value == null
              ? null
              : scrubSentryText(exception.value!),
        ),
      )
      .toList(growable: false),
);
