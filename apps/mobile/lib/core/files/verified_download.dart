import 'dart:io';
import 'package:crypto/crypto.dart';
import 'package:dio/dio.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';
import 'package:uuid/uuid.dart';
import '../network/api_client.dart';

typedef SignedUrlDownloader =
    Future<void> Function(
      String url,
      String destinationPath, {
      ProgressCallback? onReceiveProgress,
      CancelToken? cancelToken,
    });

Future<File> saveVerifiedDownloadAndAck({
  required Future<File> Function() save,
  required Future<void> Function() acknowledge,
  CancelToken? cancelToken,
}) async {
  final file = await save();
  try {
    _throwIfCancelled(cancelToken);
    await acknowledge();
    return file;
  } catch (_) {
    if (cancelToken?.isCancelled ?? false) {
      if (await file.exists()) await file.delete();
    }
    rethrow;
  }
}

class VerifiedDownload {
  static Future<Digest> sha256(File file, {CancelToken? cancelToken}) =>
      _sha256File(file, cancelToken);

  static void throwIfCancelled(CancelToken? cancelToken) =>
      _throwIfCancelled(cancelToken);

  static Future<File> copyTemporaryWithoutOverwrite({
    required File temporary,
    required Directory directory,
    required String fileName,
    required String expectedSha256,
    CancelToken? cancelToken,
  }) => _copyVerifiedWithoutOverwrite(
    temporary,
    directory,
    fileName,
    expectedSha256,
    cancelToken,
  );

  static Future<File> save({
    ApiClient? api,
    required String url,
    required String expectedSha256,
    required String fileName,
    CancelToken? cancelToken,
    ProgressCallback? onProgress,
    Directory? directory,
    SignedUrlDownloader? downloader,
  }) async {
    final destinationDirectory =
        directory ?? await getApplicationDocumentsDirectory();
    final temporary = File(
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
        temporary.path,
        onReceiveProgress: onProgress,
        cancelToken: cancelToken,
      );
      _throwIfCancelled(cancelToken);
      final digest = await _sha256File(temporary, cancelToken);
      _throwIfCancelled(cancelToken);
      if (digest.toString() != expectedSha256) {
        throw StateError('CHECKSUM_MISMATCH');
      }
      return await _copyVerifiedWithoutOverwrite(
        temporary,
        destinationDirectory,
        fileName,
        expectedSha256,
        cancelToken,
      );
    } catch (_) {
      if (await temporary.exists()) await temporary.delete();
      rethrow;
    }
  }

  static Future<File> _copyVerifiedWithoutOverwrite(
    File temporary,
    Directory directory,
    String fileName,
    String expectedSha256,
    CancelToken? cancelToken,
  ) async {
    final safeName = p.basename(fileName);
    final extension = p.extension(safeName);
    final baseName = p.basenameWithoutExtension(safeName);
    for (var suffix = 0; suffix < 10000; suffix++) {
      _throwIfCancelled(cancelToken);
      final candidateName = suffix == 0
          ? safeName
          : '$baseName ($suffix)$extension';
      final candidate = File(p.join(directory.path, candidateName));
      try {
        await candidate.create(exclusive: true);
      } on FileSystemException {
        final type = await FileSystemEntity.type(
          candidate.path,
          followLinks: false,
        );
        if (type != FileSystemEntityType.notFound) continue;
        rethrow;
      }
      late RandomAccessFile target;
      try {
        target = await candidate.open(mode: FileMode.writeOnly);
      } catch (_) {
        if (await candidate.exists()) await candidate.delete();
        rethrow;
      }
      var targetClosed = false;
      try {
        await for (final bytes in temporary.openRead()) {
          _throwIfCancelled(cancelToken);
          await target.writeFrom(bytes);
        }
        _throwIfCancelled(cancelToken);
        await target.flush();
        await target.close();
        targetClosed = true;
        final copiedDigest = await _sha256File(candidate, cancelToken);
        _throwIfCancelled(cancelToken);
        if (copiedDigest.toString() != expectedSha256) {
          throw StateError('CHECKSUM_MISMATCH');
        }
        await temporary.delete();
        return candidate;
      } catch (_) {
        if (!targetClosed) {
          try {
            await target.close();
          } catch (_) {
            // Cleanup continues even if the failed write also closed the file.
          }
        }
        if (await candidate.exists()) await candidate.delete();
        rethrow;
      }
    }
    throw StateError('DESTINATION_NAME_EXHAUSTED');
  }
}

Future<Digest> _sha256File(File file, CancelToken? cancelToken) async {
  final collector = _DigestCollector();
  final sink = sha256.startChunkedConversion(collector);
  await for (final bytes in file.openRead()) {
    _throwIfCancelled(cancelToken);
    sink.add(bytes);
  }
  sink.close();
  _throwIfCancelled(cancelToken);
  return collector.digest ?? (throw StateError('CHECKSUM_NOT_PRODUCED'));
}

void _throwIfCancelled(CancelToken? cancelToken) {
  if (cancelToken?.isCancelled ?? false) {
    throw cancelToken!.cancelError!;
  }
}

class _DigestCollector implements Sink<Digest> {
  Digest? digest;

  @override
  void add(Digest data) => digest = data;

  @override
  void close() {}
}
