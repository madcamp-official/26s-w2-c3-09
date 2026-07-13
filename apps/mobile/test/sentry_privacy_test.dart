import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/observability/sentry_privacy.dart';
import 'package:sentry_flutter/sentry_flutter.dart';

void main() {
  test('mobile Sentry event drops request data and absolute paths', () {
    final scrubbed = scrubMobileSentryEvent(
      SentryEvent(
        message: SentryMessage(
          r'Bearer private-token at C:\Users\owner\private.txt',
        ),
        request: SentryRequest(url: 'https://example.test/file?token=secret'),
        user: SentryUser(id: 'private-user'),
        exceptions: [
          SentryException(
            type: 'FileSystemException',
            value: 'failed at /tmp/private-file',
          ),
        ],
      ),
      Hint(),
    );

    final encoded = scrubbed.toJson().toString();
    expect(encoded, isNot(contains('private-token')));
    expect(encoded, isNot(contains('private.txt')));
    expect(encoded, isNot(contains('private-file')));
    expect(encoded, isNot(contains('example.test')));
    expect(scrubbed.message?.formatted, contains('[REDACTED_PATH]'));
  });
}
