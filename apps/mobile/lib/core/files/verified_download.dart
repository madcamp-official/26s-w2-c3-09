import 'dart:io';
import 'package:crypto/crypto.dart';
import 'package:dio/dio.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';
import 'package:uuid/uuid.dart';
import '../network/api_client.dart';

class VerifiedDownload {
  static Future<File> save({
    required ApiClient api,
    required String url,
    required String expectedSha256,
    required String fileName,
    CancelToken? cancelToken,
    ProgressCallback? onProgress,
  }) async {
    final directory = await getApplicationDocumentsDirectory();
    final destination = await _availableDestination(directory, fileName);
    final temporary = File(
      p.join(directory.path, '.housemouse-${const Uuid().v4()}.part'),
    );
    try {
      await api.downloadSignedUrl(
        url,
        temporary.path,
        onReceiveProgress: onProgress,
        cancelToken: cancelToken,
      );
      final digest = await sha256.bind(temporary.openRead()).first;
      if (digest.toString() != expectedSha256) {
        throw StateError('CHECKSUM_MISMATCH');
      }
      return temporary.rename(destination.path);
    } catch (_) {
      if (await temporary.exists()) await temporary.delete();
      rethrow;
    }
  }

  static Future<File> _availableDestination(
    Directory directory,
    String fileName,
  ) async {
    final safeName = p.basename(fileName);
    final extension = p.extension(safeName);
    final baseName = p.basenameWithoutExtension(safeName);
    for (var suffix = 0; suffix < 10000; suffix++) {
      final candidateName = suffix == 0
          ? safeName
          : '$baseName ($suffix)$extension';
      final candidate = File(p.join(directory.path, candidateName));
      if (!await candidate.exists()) return candidate;
    }
    throw StateError('DESTINATION_NAME_EXHAUSTED');
  }
}
