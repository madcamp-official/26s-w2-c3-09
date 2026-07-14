import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/files/smart_cache_page.dart';

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

  test(
    'download completion access event is explicit and scoped to one file',
    () {
      expect(
        smartCacheAccessEventPath('cached-file-1'),
        '/v1/cached-files/cached-file-1/access-events',
      );
      expect(smartCacheDownloadCompletedAccessEvent(), {
        'eventType': 'DOWNLOAD_COMPLETED',
      });
    },
  );

  test('encrypted cached files fail closed until key sync exists', () {
    expect(
      () => ensureSmartCacheDownloadDecryptable({
        'downloadUrl': 'https://storage.example/cache/object',
        'sha256': List.filled(64, 'a').join(),
        'encryptionMetadata': {
          'algorithm': 'AES-256-GCM',
          'format': 'MKS1_NONCE_CIPHERTEXT_TAG',
          'keyId': 'mks1-test-key-1234',
        },
      }),
      throwsA(
        isA<StateError>().having(
          (error) => error.message,
          'message',
          contains('UNCONFIGURED'),
        ),
      ),
    );
    expect(
      () => ensureSmartCacheDownloadDecryptable({
        'downloadUrl': 'https://storage.example/cache/object',
        'sha256': List.filled(64, 'a').join(),
      }),
      returnsNormally,
    );
  });
}
