import 'package:flutter_test/flutter_test.dart';
import 'package:housemouse/features/files/smart_cache_page.dart';

void main() {
  test('opt-in policy input converts MB and preserves explicit exclusions', () {
    expect(
      parseSmartCachePolicyInput(
        quotaMegabytes: '500',
        maxFileMegabytes: '50',
        excludedPatterns: 'private/**\n*.tmp\n',
        enabled: true,
      ),
      {
        'enabled': true,
        'quotaBytes': 500 * 1024 * 1024,
        'maxFileBytes': 50 * 1024 * 1024,
        'excludedPatterns': ['private/**', '*.tmp'],
      },
    );
  });

  test('file limit cannot exceed the room quota', () {
    expect(
      () => parseSmartCachePolicyInput(
        quotaMegabytes: '10',
        maxFileMegabytes: '20',
        excludedPatterns: '',
        enabled: true,
      ),
      throwsFormatException,
    );
  });
}
