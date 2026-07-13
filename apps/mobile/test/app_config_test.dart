import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/config/app_config.dart';

void main() {
  test('Sentry DSN validation accepts only HTTPS URLs', () {
    expect(
      isValidSentryDsn('https://public@example.ingest.sentry.io/1'),
      isTrue,
    );
    expect(isValidSentryDsn('http://example.test/1'), isFalse);
    expect(isValidSentryDsn('not-a-dsn'), isFalse);
  });
}
