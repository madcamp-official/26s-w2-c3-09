import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:dio/dio.dart';
import '../../core/files/smart_cache_decryption.dart';
import '../../core/network/api_client.dart';
import '../../storage/display_cache.dart';

String smartCacheAccessEventPath(String cachedFileId) =>
    '/v1/cached-files/$cachedFileId/access-events';

String smartCacheFilesPath(String roomId) =>
    '/v1/rooms/$roomId/smart-cache/files';

Map<String, dynamic> smartCacheDownloadCompletedAccessEvent() => const {
  'eventType': 'DOWNLOAD_COMPLETED',
};

Future<void> recordSmartCacheDownloadCompleted(
  ApiClient api,
  String cachedFileId,
) {
  return api.post(
    smartCacheAccessEventPath(cachedFileId),
    smartCacheDownloadCompletedAccessEvent(),
  );
}

List<Map<String, dynamic>> smartCacheFilesFromPayload(
  Map<String, dynamic> cache,
) {
  final raw = cache['files'];
  if (raw == null) return const [];
  if (raw is! List) {
    throw const FormatException('INVALID_SMART_CACHE_FILES');
  }
  return raw
      .map((item) {
        if (item is! Map) {
          throw const FormatException('INVALID_SMART_CACHE_FILES');
        }
        return Map<String, dynamic>.from(item);
      })
      .toList(growable: false);
}

Map<String, dynamic> smartCacheOfflineFallbackPayload(
  List<Map<String, dynamic>> files,
) {
  return {
    'files': files,
    'pendingCommandWarning': false,
    'desktopOnline': false,
    'offlineFallback': true,
  };
}

bool isSmartCacheOfflineFallbackError(Object error) {
  if (error is! DioException) return false;
  return switch (error.type) {
    DioExceptionType.connectionError ||
    DioExceptionType.connectionTimeout ||
    DioExceptionType.receiveTimeout ||
    DioExceptionType.sendTimeout => true,
    _ => false,
  };
}

class SmartCachePageContent {
  const SmartCachePageContent({required this.cache, this.offlineFallbackError});

  final Map<String, dynamic> cache;
  final Object? offlineFallbackError;

  bool get isOfflineFallback => offlineFallbackError != null;
}

typedef SmartCacheGet = Future<Map<String, dynamic>> Function(String path);

final smartCacheGetProvider = Provider<SmartCacheGet>((ref) {
  final api = ref.watch(apiClientProvider);
  return api.get;
});

final smartCacheDecryptionKeyStoreProvider =
    Provider<SmartCacheDecryptionKeyStore>(
      (ref) => const UnconfiguredSmartCacheDecryptionKeyStore(),
    );

final smartCacheFilesProvider = FutureProvider.autoDispose
    .family<Map<String, dynamic>, String>(
      (ref, roomId) =>
          ref.watch(smartCacheGetProvider)(smartCacheFilesPath(roomId)),
    );

final smartCacheStatusProvider = FutureProvider.autoDispose
    .family<SmartCachePageContent, String>((ref, roomId) async {
      final displayCache = ref.watch(displayCacheProvider);
      try {
        final cache = await ref.watch(smartCacheFilesProvider(roomId).future);
        final files = smartCacheFilesFromPayload(cache);
        await displayCache.replaceSmartCacheFiles(roomId, files);
        return SmartCachePageContent(cache: cache);
      } catch (error) {
        if (!isSmartCacheOfflineFallbackError(error)) rethrow;
        final localFiles = await displayCache.smartCacheFiles(roomId);
        if (localFiles.isEmpty) rethrow;
        return SmartCachePageContent(
          cache: smartCacheOfflineFallbackPayload(localFiles),
          offlineFallbackError: error,
        );
      }
    });

class SmartCachePage extends ConsumerStatefulWidget {
  const SmartCachePage({super.key, required this.roomId});
  final String roomId;
  @override
  ConsumerState<SmartCachePage> createState() => _SmartCachePageState();
}

class _SmartCachePageState extends ConsumerState<SmartCachePage> {
  late Future<SmartCachePageContent> content;
  String? downloadingId;
  double? downloadProgress;
  @override
  void initState() {
    super.initState();
    reload();
  }

  void reload() {
    ref.invalidate(smartCacheStatusProvider(widget.roomId));
    content = ref.read(smartCacheStatusProvider(widget.roomId).future);
  }

  Future<void> download(Map<String, dynamic> file) async {
    final id = file['id'] as String;
    setState(() {
      downloadingId = id;
      downloadProgress = null;
    });
    try {
      final api = ref.read(apiClientProvider);
      final target = await api.get('/v1/cached-files/$id/download');
      final saved = await saveSmartCacheDownload(
        api: api,
        target: target,
        file: file,
        keyStore: ref.read(smartCacheDecryptionKeyStoreProvider),
        onProgress: (received, total) {
          if (mounted && total > 0) {
            setState(() => downloadProgress = received / total);
          }
        },
      );
      Object? localMetadataError;
      try {
        await ref
            .read(displayCacheProvider)
            .markSmartCacheFileDownloaded(
              roomId: widget.roomId,
              file: file,
              downloadTarget: target,
              localDownloadPath: saved.path,
            );
      } catch (error) {
        localMetadataError = error;
      }
      Object? accessEventError;
      try {
        await recordSmartCacheDownloadCompleted(api, id);
      } catch (error) {
        accessEventError = error;
      }
      if (mounted && (localMetadataError != null || accessEventError != null)) {
        final warnings = [
          if (localMetadataError != null) '로컬 캐시 기록 실패: $localMetadataError',
          if (accessEventError != null) '사용 기록 동기화 실패: $accessEventError',
        ].join(' / ');
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('checksum 확인 후 저장 완료, $warnings')),
        );
        return;
      }
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('checksum 확인 후 저장 완료: ${saved.path}')),
        );
      }
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('캐시 파일 다운로드 실패: $error')));
      }
    } finally {
      if (mounted) {
        setState(() {
          downloadingId = null;
          downloadProgress = null;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('오프라인 파일')),
    body: FutureBuilder<SmartCachePageContent>(
      future: content,
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const Center(child: CircularProgressIndicator());
        }
        if (snapshot.hasError) {
          return Center(
            child: Text(
              '스마트 캐시를 불러오지 못했습니다.\n${snapshot.error}',
              textAlign: TextAlign.center,
            ),
          );
        }
        final data = snapshot.data!;
        final cache = data.cache;
        final files = smartCacheFilesFromPayload(cache);
        return ListView(
          padding: const EdgeInsets.all(16),
          children: [
            if (data.isOfflineFallback)
              SmartCacheOfflineFallbackWarning(
                error: data.offlineFallbackError,
              ),
            if (cache['pendingCommandWarning'] == true)
              const PendingCommandWarning(),
            if (cache['desktopOnline'] == false)
              const DesktopOfflineCacheWarning(),
            const Divider(),
            if (files.isEmpty)
              const ListTile(title: Text('오프라인에서 사용할 수 있는 캐시 파일이 없습니다'))
            else
              ...files.map((file) {
                final isDownloading = downloadingId == file['id'];
                return CachedFileTile(
                  file: file,
                  isDownloading: isDownloading,
                  progress: downloadProgress,
                  onDownload: () => download(file),
                );
              }),
          ],
        );
      },
    ),
  );
}

class PendingCommandWarning extends StatelessWidget {
  const PendingCommandWarning({super.key});

  @override
  Widget build(BuildContext context) => const Card(
    color: Colors.amberAccent,
    child: ListTile(
      leading: Icon(Icons.warning_amber),
      title: Text('명령 처리 후 파일 위치나 목록이 변경될 수 있습니다.'),
    ),
  );
}

class SmartCacheOfflineFallbackWarning extends StatelessWidget {
  const SmartCacheOfflineFallbackWarning({super.key, this.error});

  final Object? error;

  @override
  Widget build(BuildContext context) => Card(
    color: const Color(0xFFFFF3E0),
    child: ListTile(
      leading: const Icon(Icons.cloud_off_outlined),
      title: const Text('Offline smart-cache metadata'),
      subtitle: Text(
        'Showing the last verified local cache metadata because the server is unreachable.'
        '${error == null ? '' : '\n$error'}',
      ),
    ),
  );
}

class DesktopOfflineCacheWarning extends StatelessWidget {
  const DesktopOfflineCacheWarning({super.key});

  @override
  Widget build(BuildContext context) => const Card(
    color: Color(0xFFFFF3E0),
    child: ListTile(
      leading: Icon(Icons.computer_outlined),
      title: Text('PC 에이전트와 연결되지 않음'),
      subtitle: Text('다운로드는 가능하지만 원본의 최신 상태를 다시 확인할 수 없습니다.'),
    ),
  );
}

class CachedFileTile extends StatelessWidget {
  const CachedFileTile({
    super.key,
    required this.file,
    required this.isDownloading,
    required this.progress,
    required this.onDownload,
  });
  final Map<String, dynamic> file;
  final bool isDownloading;
  final double? progress;
  final VoidCallback onDownload;

  @override
  Widget build(BuildContext context) => Card(
    child: ListTile(
      leading: isDownloading
          ? CircularProgressIndicator(value: progress)
          : const Icon(Icons.offline_pin),
      title: Text(file['sourceRelativePath'] as String),
      subtitle: Text(
        '${_availabilityLabel(file['availabilityStatus'] as String?)} · '
        '${_freshnessLabel(file['freshnessStatus'] as String?)}\n'
        '캐시 시각: ${file['cachedAt']}\n'
        '원본 마지막 확인: ${file['lastVerifiedAt']}',
      ),
      trailing: IconButton(
        tooltip: '다운로드',
        onPressed: isDownloading ? null : onDownload,
        icon: const Icon(Icons.download_outlined),
      ),
    ),
  );

  String _availabilityLabel(String? status) {
    return status == 'AVAILABLE' ? '다운로드 가능' : '사용 불가';
  }

  String _freshnessLabel(String? status) {
    return switch (status) {
      'VERIFIED_CURRENT' => '최신 확인됨',
      'UNVERIFIED_OFFLINE' => 'PC 오프라인·최신 여부 미확인',
      'STALE' => '원본 변경됨',
      _ => '최신 여부 알 수 없음',
    };
  }
}
