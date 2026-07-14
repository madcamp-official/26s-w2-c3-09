import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:cryptography/cryptography.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mousekeeper/core/files/smart_cache_decryption.dart';
import 'package:mousekeeper/features/files/smart_cache_page.dart';

void main() {
  test('opt-in policy input converts MB and preserves explicit exclusions', () {
    expect(
      parseSmartCachePolicyInput(
        quotaMegabytes: '500',
        maxFileMegabytes: '50',
        excludedPatterns: 'private/**\n*.tmp\n',
        pinnedPatterns: 'important/**\n',
        enabled: true,
      ),
      {
        'enabled': true,
        'quotaBytes': 500 * 1024 * 1024,
        'maxFileBytes': 50 * 1024 * 1024,
        'excludedPatterns': ['private/**', '*.tmp'],
        'pinnedPatterns': ['important/**'],
      },
    );
  });

  test('file limit cannot exceed the room quota', () {
    expect(
      () => parseSmartCachePolicyInput(
        quotaMegabytes: '10',
        maxFileMegabytes: '20',
        excludedPatterns: '',
        pinnedPatterns: '',
        enabled: true,
      ),
      throwsFormatException,
    );
  });

  test(
    'download completion access event is explicit and scoped to one file',
    () {
      expect(
        smartCacheFilesPath('room-1'),
        '/v1/rooms/room-1/smart-cache/files',
      );
      expect(
        smartCacheAccessEventPath('cached-file-1'),
        '/v1/cached-files/cached-file-1/access-events',
      );
      expect(smartCacheDownloadCompletedAccessEvent(), {
        'eventType': 'DOWNLOAD_COMPLETED',
      });
    },
  );

  test(
    'smart-cache read providers request policy and file projections',
    () async {
      final paths = <String>[];
      final container = ProviderContainer(
        overrides: [
          smartCacheGetProvider.overrideWithValue((path) async {
            paths.add(path);
            if (path.endsWith('/smart-cache-policy')) {
              return {'enabled': true};
            }
            return {'files': const []};
          }),
        ],
      );
      addTearDown(container.dispose);

      final policy = await container.read(
        smartCachePolicyProvider('room-1').future,
      );
      final files = await container.read(
        smartCacheFilesProvider('room-1').future,
      );

      expect(policy, {'enabled': true});
      expect(files, {'files': const []});
      expect(paths, [
        '/v1/rooms/room-1/smart-cache-policy',
        '/v1/rooms/room-1/smart-cache/files',
      ]);
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
          'nonceHex': '030303030303030303030303',
          'plaintextSizeBytes': 17,
          'plaintextSha256': List.filled(64, 'b').join(),
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

  test(
    'encrypted smart-cache download verifies tag and stores plaintext only',
    () async {
      final temp = await Directory.systemTemp.createTemp(
        'mousekeeper-smart-cache-',
      );
      addTearDown(() => temp.delete(recursive: true));

      final plaintext = utf8.encode('mousekeeper cached plaintext');
      final key = List<int>.generate(32, (index) => index);
      final nonce = List<int>.filled(12, 3);
      final encrypted = await _encryptedEnvelope(
        plaintext: plaintext,
        key: key,
        nonce: nonce,
      );
      final target = _encryptedTarget(
        encrypted,
        plaintext: plaintext,
        nonce: nonce,
      );

      final saved = await saveSmartCacheDownload(
        target: target,
        file: const {'sourceRelativePath': 'reports/final.txt'},
        keyStore: _StaticSmartCacheKeyStore(
          SmartCacheDecryptionKey(
            keyId: target['encryptionMetadata']['keyId'] as String,
            bytes: key,
          ),
        ),
        directory: temp,
        downloader:
            (url, destinationPath, {onReceiveProgress, cancelToken}) async {
              expect(url, 'https://storage.example/cache/object');
              await File(destinationPath).writeAsBytes(encrypted);
            },
      );

      expect(await saved.readAsString(), 'mousekeeper cached plaintext');
      expect(saved.path.endsWith('final.txt'), isTrue);
      final savedBytes = await saved.readAsBytes();
      expect(savedBytes.sublist(0, 4), isNot(utf8.encode('MKS1')));
    },
  );

  test('encrypted smart-cache download rejects a tampered auth tag', () async {
    final temp = await Directory.systemTemp.createTemp(
      'mousekeeper-smart-cache-tampered-',
    );
    addTearDown(() => temp.delete(recursive: true));

    final plaintext = utf8.encode('mousekeeper cached plaintext');
    final key = List<int>.generate(32, (index) => index);
    final nonce = List<int>.filled(12, 3);
    final encrypted = await _encryptedEnvelope(
      plaintext: plaintext,
      key: key,
      nonce: nonce,
    );
    final tampered = List<int>.from(encrypted);
    tampered[tampered.length - 1] ^= 1;

    await expectLater(
      saveSmartCacheDownload(
        target: _encryptedTarget(tampered, plaintext: plaintext, nonce: nonce),
        file: const {'sourceRelativePath': 'reports/final.txt'},
        keyStore: _StaticSmartCacheKeyStore(
          SmartCacheDecryptionKey(keyId: 'mks1-test-key-1234', bytes: key),
        ),
        directory: temp,
        downloader:
            (url, destinationPath, {onReceiveProgress, cancelToken}) async {
              await File(destinationPath).writeAsBytes(tampered);
            },
      ),
      throwsA(
        isA<StateError>().having(
          (error) => error.message,
          'message',
          'SMART_CACHE_AUTHENTICATION_FAILED',
        ),
      ),
    );
    expect(temp.listSync(), isEmpty);
  });

  test('offline fallback payload preserves verified local cache metadata', () {
    final files = [
      {
        'id': 'cached-file-1',
        'sourceRelativePath': 'reports/final.pdf',
        'availabilityStatus': 'AVAILABLE',
        'freshnessStatus': 'UNVERIFIED_OFFLINE',
        'localDownloadPath': '/local/reports/final.pdf',
        'sha256': List.filled(64, 'a').join(),
        'lastVerifiedAt': '2026-07-14T02:03:04.000Z',
      },
    ];

    expect(smartCacheFilesFromPayload({'files': files}), files);
    expect(smartCacheOfflineFallbackPayload(files), {
      'files': files,
      'pendingCommandWarning': false,
      'desktopOnline': false,
      'offlineFallback': true,
    });
  });

  test('smart-cache offline fallback is limited to transport failures', () {
    final connectionError = DioException(
      requestOptions: RequestOptions(
        path: '/v1/rooms/room-a/smart-cache/files',
      ),
      type: DioExceptionType.connectionError,
    );
    final serverError = DioException(
      requestOptions: RequestOptions(
        path: '/v1/rooms/room-a/smart-cache/files',
      ),
      response: Response<Map<String, dynamic>>(
        requestOptions: RequestOptions(
          path: '/v1/rooms/room-a/smart-cache/files',
        ),
        statusCode: 500,
        data: const {'code': 'INTERNAL_SERVER_ERROR'},
      ),
      type: DioExceptionType.badResponse,
    );

    expect(isSmartCacheOfflineFallbackError(connectionError), isTrue);
    expect(isSmartCacheOfflineFallbackError(serverError), isFalse);
    expect(
      () => smartCacheFilesFromPayload({
        'files': ['not-a-file-map'],
      }),
      throwsFormatException,
    );
  });
}

class _StaticSmartCacheKeyStore implements SmartCacheDecryptionKeyStore {
  const _StaticSmartCacheKeyStore(this.key);

  final SmartCacheDecryptionKey key;

  @override
  Future<SmartCacheDecryptionKey?> keyFor(
    SmartCacheEncryptionMetadata metadata,
  ) async => key;
}

Future<List<int>> _encryptedEnvelope({
  required List<int> plaintext,
  required List<int> key,
  required List<int> nonce,
}) async {
  final secretBox = await AesGcm.with256bits().encrypt(
    plaintext,
    secretKey: SecretKey(key),
    nonce: nonce,
  );
  return [
    ...utf8.encode('MKS1'),
    ...nonce,
    ...secretBox.cipherText,
    ...secretBox.mac.bytes,
  ];
}

Map<String, dynamic> _encryptedTarget(
  List<int> ciphertext, {
  required List<int> plaintext,
  required List<int> nonce,
}) {
  return {
    'downloadUrl': 'https://storage.example/cache/object',
    'sha256': sha256.convert(ciphertext).toString(),
    'encryptionMetadata': {
      'algorithm': 'AES-256-GCM',
      'format': 'MKS1_NONCE_CIPHERTEXT_TAG',
      'keyId': 'mks1-test-key-1234',
      'nonceHex': nonce
          .map((byte) => byte.toRadixString(16).padLeft(2, '0'))
          .join(),
      'plaintextSizeBytes': plaintext.length,
      'plaintextSha256': sha256.convert(plaintext).toString(),
    },
  };
}
