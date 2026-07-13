import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/files/verified_download.dart';

void main() {
  late Directory directory;

  setUp(() async {
    directory = await Directory.systemTemp.createTemp('mousekeeper-download-');
  });

  tearDown(() async {
    if (await directory.exists()) await directory.delete(recursive: true);
  });

  test('valid content is written to .part, verified, then renamed', () async {
    final bytes = 'verified mousekeeper file'.codeUnits;
    String? downloadPath;

    final saved = await VerifiedDownload.save(
      directory: directory,
      url: 'https://signed.example/file',
      expectedSha256: sha256.convert(bytes).toString(),
      fileName: 'nested/report.txt',
      downloader:
          (
            String _,
            String destinationPath, {
            ProgressCallback? onReceiveProgress,
            CancelToken? cancelToken,
          }) async {
            downloadPath = destinationPath;
            expect(destinationPath, endsWith('.part'));
            await File(destinationPath).writeAsBytes(bytes);
          },
    );

    expect(downloadPath, isNotNull);
    expect(saved.path, endsWith('report.txt'));
    expect(await saved.readAsBytes(), bytes);
    expect(await File(downloadPath!).exists(), isFalse);
  });

  test('checksum mismatch deletes .part and creates no final file', () async {
    final bytes = 'tampered'.codeUnits;
    String? temporaryPath;

    await expectLater(
      VerifiedDownload.save(
        directory: directory,
        url: 'https://signed.example/file',
        expectedSha256: sha256.convert('expected'.codeUnits).toString(),
        fileName: 'report.txt',
        downloader:
            (
              String _,
              String destinationPath, {
              ProgressCallback? onReceiveProgress,
              CancelToken? cancelToken,
            }) async {
              temporaryPath = destinationPath;
              await File(destinationPath).writeAsBytes(bytes);
            },
      ),
      throwsA(isA<StateError>()),
    );

    expect(await File(temporaryPath!).exists(), isFalse);
    expect(
      await File(
        '${directory.path}${Platform.pathSeparator}report.txt',
      ).exists(),
      isFalse,
    );
  });

  test('a destination created during download is never overwritten', () async {
    final bytes = 'new verified content'.codeUnits;
    final existing = File(
      '${directory.path}${Platform.pathSeparator}report.txt',
    );

    final saved = await VerifiedDownload.save(
      directory: directory,
      url: 'https://signed.example/file',
      expectedSha256: sha256.convert(bytes).toString(),
      fileName: 'report.txt',
      downloader:
          (
            String _,
            String destinationPath, {
            ProgressCallback? onReceiveProgress,
            CancelToken? cancelToken,
          }) async {
            await File(destinationPath).writeAsBytes(bytes);
            await existing.writeAsString('keep me');
          },
    );

    expect(await existing.readAsString(), 'keep me');
    expect(saved.path, endsWith('report (1).txt'));
    expect(await saved.readAsBytes(), bytes);
  });

  test('cancellation during final copy removes candidate and .part', () async {
    final bytes = Uint8List(16 * 1024 * 1024)
      ..fillRange(0, 16 * 1024 * 1024, 7);
    final token = CancelToken();
    final candidate = File(
      '${directory.path}${Platform.pathSeparator}large.bin',
    );
    final watcher = Timer.periodic(const Duration(milliseconds: 1), (timer) {
      if (candidate.existsSync()) {
        token.cancel('room removed');
        timer.cancel();
      }
    });

    try {
      await expectLater(
        VerifiedDownload.save(
          directory: directory,
          url: 'https://signed.example/file',
          expectedSha256: sha256.convert(bytes).toString(),
          fileName: 'large.bin',
          cancelToken: token,
          downloader:
              (
                String _,
                String destinationPath, {
                ProgressCallback? onReceiveProgress,
                CancelToken? cancelToken,
              }) => File(destinationPath).writeAsBytes(bytes),
        ),
        throwsA(isA<DioException>()),
      );
    } finally {
      watcher.cancel();
    }

    expect(await candidate.exists(), isFalse);
    expect(
      await directory
          .list()
          .where((entry) => entry.path.endsWith('.part'))
          .toList(),
      isEmpty,
    );
  });

  test('ACK runs only after verified save succeeds', () async {
    final order = <String>[];
    final source = File(
      '${directory.path}${Platform.pathSeparator}completed.txt',
    );

    final saved = await saveVerifiedDownloadAndAck(
      save: () async {
        order.add('verified-save');
        await source.writeAsString('done');
        return source;
      },
      acknowledge: () async => order.add('ack'),
    );

    expect(saved.path, source.path);
    expect(order, ['verified-save', 'ack']);

    var acknowledgedAfterFailure = false;
    await expectLater(
      saveVerifiedDownloadAndAck(
        save: () async => throw StateError('CHECKSUM_MISMATCH'),
        acknowledge: () async => acknowledgedAfterFailure = true,
      ),
      throwsStateError,
    );
    expect(acknowledgedAfterFailure, isFalse);

    final cancelledFile = File(
      '${directory.path}${Platform.pathSeparator}cancelled.txt',
    );
    final cancelToken = CancelToken()..cancel('room removed');
    var acknowledgedAfterCancel = false;
    await expectLater(
      saveVerifiedDownloadAndAck(
        cancelToken: cancelToken,
        save: () async {
          await cancelledFile.writeAsString('verified');
          return cancelledFile;
        },
        acknowledge: () async => acknowledgedAfterCancel = true,
      ),
      throwsA(isA<DioException>()),
    );
    expect(acknowledgedAfterCancel, isFalse);
    expect(await cancelledFile.exists(), isFalse);
  });
}
