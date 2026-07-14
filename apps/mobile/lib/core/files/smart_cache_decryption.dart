import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:cryptography/cryptography.dart';
import 'package:dio/dio.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';
import 'package:uuid/uuid.dart';

import '../network/api_client.dart';
import 'verified_download.dart';

const smartCacheDecryptionUnconfiguredMessage =
    'UNCONFIGURED: SMART_CACHE_DECRYPTION_KEY_SYNC';

const _algorithm = 'AES-256-GCM';
const _format = 'MKS1_NONCE_CIPHERTEXT_TAG';
const _magic = [0x4d, 0x4b, 0x53, 0x31]; // MKS1
const _nonceBytes = 12;
const _tagBytes = 16;
const _keyBytes = 32;

class SmartCacheEncryptionMetadata {
  const SmartCacheEncryptionMetadata({
    required this.algorithm,
    required this.format,
    required this.keyId,
    required this.nonceHex,
    required this.plaintextSizeBytes,
    required this.plaintextSha256,
  });

  final String algorithm;
  final String format;
  final String keyId;
  final String nonceHex;
  final int plaintextSizeBytes;
  final String plaintextSha256;

  factory SmartCacheEncryptionMetadata.fromJson(Object? value) {
    if (value is! Map) {
      throw const FormatException('INVALID_SMART_CACHE_ENCRYPTION_METADATA');
    }
    final map = Map<String, dynamic>.from(value);
    final parsed = SmartCacheEncryptionMetadata(
      algorithm: _stringField(map, 'algorithm'),
      format: _stringField(map, 'format'),
      keyId: _stringField(map, 'keyId'),
      nonceHex: _hexField(map, 'nonceHex', _nonceBytes),
      plaintextSizeBytes: _intField(map, 'plaintextSizeBytes'),
      plaintextSha256: _hexField(map, 'plaintextSha256', 32),
    );
    if (parsed.algorithm != _algorithm) {
      throw const FormatException(
        'UNSUPPORTED_SMART_CACHE_ENCRYPTION_ALGORITHM',
      );
    }
    if (parsed.format != _format) {
      throw const FormatException('UNSUPPORTED_SMART_CACHE_ENCRYPTION_FORMAT');
    }
    if (parsed.keyId.trim().isEmpty) {
      throw const FormatException('INVALID_SMART_CACHE_KEY_ID');
    }
    return parsed;
  }
}

class SmartCacheDecryptionKey {
  const SmartCacheDecryptionKey({required this.keyId, required this.bytes});

  final String keyId;
  final List<int> bytes;
}

abstract interface class SmartCacheDecryptionKeyStore {
  Future<SmartCacheDecryptionKey?> keyFor(
    SmartCacheEncryptionMetadata metadata,
  );
}

class UnconfiguredSmartCacheDecryptionKeyStore
    implements SmartCacheDecryptionKeyStore {
  const UnconfiguredSmartCacheDecryptionKeyStore();

  @override
  Future<SmartCacheDecryptionKey?> keyFor(
    SmartCacheEncryptionMetadata metadata,
  ) async => null;
}

void ensureSmartCacheDownloadDecryptable(Map<String, dynamic> target) {
  final metadata = target['encryptionMetadata'];
  if (metadata == null) return;
  SmartCacheEncryptionMetadata.fromJson(metadata);
  throw StateError(smartCacheDecryptionUnconfiguredMessage);
}

Future<File> saveSmartCacheDownload({
  ApiClient? api,
  required Map<String, dynamic> target,
  required Map<String, dynamic> file,
  required SmartCacheDecryptionKeyStore keyStore,
  CancelToken? cancelToken,
  ProgressCallback? onProgress,
  Directory? directory,
  SignedUrlDownloader? downloader,
}) async {
  final url = _stringField(target, 'downloadUrl');
  final ciphertextSha256 = _hexField(target, 'sha256', 32);
  final fileName = _stringField(file, 'sourceRelativePath');
  final metadata = target['encryptionMetadata'];
  if (metadata == null) {
    return VerifiedDownload.save(
      api: api,
      url: url,
      expectedSha256: ciphertextSha256,
      fileName: fileName,
      cancelToken: cancelToken,
      onProgress: onProgress,
      directory: directory,
      downloader: downloader,
    );
  }

  final parsedMetadata = SmartCacheEncryptionMetadata.fromJson(metadata);
  final key = await keyStore.keyFor(parsedMetadata);
  if (key == null) {
    throw StateError(smartCacheDecryptionUnconfiguredMessage);
  }
  if (key.keyId != parsedMetadata.keyId) {
    throw StateError('SMART_CACHE_KEY_ID_MISMATCH');
  }
  if (key.bytes.length != _keyBytes) {
    throw StateError('INVALID_SMART_CACHE_DECRYPTION_KEY');
  }

  return EncryptedSmartCacheDownload.save(
    api: api,
    url: url,
    expectedCiphertextSha256: ciphertextSha256,
    fileName: fileName,
    metadata: parsedMetadata,
    keyBytes: key.bytes,
    cancelToken: cancelToken,
    onProgress: onProgress,
    directory: directory,
    downloader: downloader,
  );
}

class EncryptedSmartCacheDownload {
  static Future<File> save({
    ApiClient? api,
    required String url,
    required String expectedCiphertextSha256,
    required String fileName,
    required SmartCacheEncryptionMetadata metadata,
    required List<int> keyBytes,
    CancelToken? cancelToken,
    ProgressCallback? onProgress,
    Directory? directory,
    SignedUrlDownloader? downloader,
  }) async {
    final destinationDirectory =
        directory ?? await getApplicationDocumentsDirectory();
    final encryptedTemporary = File(
      p.join(
        destinationDirectory.path,
        '.mousekeeper-${const Uuid().v4()}.encrypted.part',
      ),
    );
    final plaintextTemporary = File(
      p.join(
        destinationDirectory.path,
        '.mousekeeper-${const Uuid().v4()}.part',
      ),
    );
    try {
      final download =
          downloader ??
          (
            String source,
            String target, {
            ProgressCallback? onReceiveProgress,
            CancelToken? cancelToken,
          }) {
            if (api == null) {
              throw ArgumentError('api or downloader is required');
            }
            return api.downloadSignedUrl(
              source,
              target,
              onReceiveProgress: onReceiveProgress,
              cancelToken: cancelToken,
            );
          };
      await download(
        url,
        encryptedTemporary.path,
        onReceiveProgress: onProgress,
        cancelToken: cancelToken,
      );
      VerifiedDownload.throwIfCancelled(cancelToken);
      final ciphertextDigest = await VerifiedDownload.sha256(
        encryptedTemporary,
        cancelToken: cancelToken,
      );
      if (ciphertextDigest.toString() != expectedCiphertextSha256) {
        throw StateError('CHECKSUM_MISMATCH');
      }

      final plaintext = await decryptSmartCacheEnvelope(
        await encryptedTemporary.readAsBytes(),
        metadata: metadata,
        keyBytes: keyBytes,
      );
      VerifiedDownload.throwIfCancelled(cancelToken);
      await plaintextTemporary.writeAsBytes(plaintext, flush: true);
      VerifiedDownload.throwIfCancelled(cancelToken);
      return await VerifiedDownload.copyTemporaryWithoutOverwrite(
        temporary: plaintextTemporary,
        directory: destinationDirectory,
        fileName: fileName,
        expectedSha256: metadata.plaintextSha256,
        cancelToken: cancelToken,
      );
    } catch (_) {
      if (await plaintextTemporary.exists()) await plaintextTemporary.delete();
      rethrow;
    } finally {
      if (await encryptedTemporary.exists()) await encryptedTemporary.delete();
    }
  }
}

Future<Uint8List> decryptSmartCacheEnvelope(
  List<int> envelope, {
  required SmartCacheEncryptionMetadata metadata,
  required List<int> keyBytes,
}) async {
  if (keyBytes.length != _keyBytes) {
    throw StateError('INVALID_SMART_CACHE_DECRYPTION_KEY');
  }
  if (envelope.length < _magic.length + _nonceBytes + _tagBytes) {
    throw const FormatException('INVALID_SMART_CACHE_ENVELOPE');
  }
  for (var index = 0; index < _magic.length; index++) {
    if (envelope[index] != _magic[index]) {
      throw const FormatException('INVALID_SMART_CACHE_ENVELOPE_MAGIC');
    }
  }
  final nonce = Uint8List.fromList(
    envelope.sublist(_magic.length, _magic.length + _nonceBytes),
  );
  if (_hexLower(nonce) != metadata.nonceHex) {
    throw StateError('SMART_CACHE_NONCE_MISMATCH');
  }
  final ciphertextStart = _magic.length + _nonceBytes;
  final ciphertextEnd = envelope.length - _tagBytes;
  final ciphertext = Uint8List.fromList(
    envelope.sublist(ciphertextStart, ciphertextEnd),
  );
  final tag = envelope.sublist(ciphertextEnd);
  late final List<int> plaintext;
  try {
    plaintext = await AesGcm.with256bits().decrypt(
      SecretBox(ciphertext, nonce: nonce, mac: Mac(tag)),
      secretKey: SecretKey(keyBytes),
    );
  } on SecretBoxAuthenticationError {
    throw StateError('SMART_CACHE_AUTHENTICATION_FAILED');
  }
  final bytes = Uint8List.fromList(plaintext);
  if (bytes.length != metadata.plaintextSizeBytes) {
    throw StateError('SMART_CACHE_PLAINTEXT_SIZE_MISMATCH');
  }
  if (sha256.convert(bytes).toString() != metadata.plaintextSha256) {
    throw StateError('SMART_CACHE_PLAINTEXT_CHECKSUM_MISMATCH');
  }
  return bytes;
}

String _stringField(Map<String, dynamic> map, String name) {
  final value = map[name];
  if (value is! String || value.trim().isEmpty) {
    throw FormatException('INVALID_SMART_CACHE_FIELD_$name');
  }
  return value;
}

int _intField(Map<String, dynamic> map, String name) {
  final value = map[name];
  final parsed = switch (value) {
    int() => value,
    num() when value.isFinite && value == value.roundToDouble() =>
      value.toInt(),
    _ => null,
  };
  if (parsed == null || parsed < 0) {
    throw FormatException('INVALID_SMART_CACHE_FIELD_$name');
  }
  return parsed;
}

String _hexField(Map<String, dynamic> map, String name, int byteLength) {
  final value = _stringField(map, name).toLowerCase();
  if (value.length != byteLength * 2 ||
      !RegExp(r'^[0-9a-f]+$').hasMatch(value)) {
    throw FormatException('INVALID_SMART_CACHE_FIELD_$name');
  }
  return value;
}

String _hexLower(List<int> bytes) =>
    bytes.map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();
