import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:dio/dio.dart';
import '../../core/files/smart_cache_decryption.dart';
import '../../core/network/api_client.dart';
import '../../storage/display_cache.dart';

Map<String, dynamic> parseSmartCachePolicyInput({
  required String quotaMegabytes,
  required String maxFileMegabytes,
  required String excludedPatterns,
  required String pinnedPatterns,
  required bool enabled,
}) {
  const megabyte = 1024 * 1024;
  final quota = int.tryParse(quotaMegabytes);
  final maxFile = int.tryParse(maxFileMegabytes);
  if (quota == null || quota <= 0 || maxFile == null || maxFile <= 0) {
    throw const FormatException('용량은 1 이상의 정수여야 합니다.');
  }
  if (maxFile > quota) {
    throw const FormatException('파일 한도는 방 한도보다 클 수 없습니다.');
  }
  final excluded = _parseSmartCachePatterns(excludedPatterns);
  final pinned = _parseSmartCachePatterns(pinnedPatterns);
  if (excluded.length > 100 ||
      pinned.length > 100 ||
      excluded.any((pattern) => pattern.length > 255) ||
      pinned.any((pattern) => pattern.length > 255)) {
    throw const FormatException('패턴은 종류별 최대 100개, 각 255자까지입니다.');
  }
  return {
    'enabled': enabled,
    'quotaBytes': quota * megabyte,
    'maxFileBytes': maxFile * megabyte,
    'excludedPatterns': excluded,
    'pinnedPatterns': pinned,
  };
}

List<String> _parseSmartCachePatterns(String value) => value
    .split('\n')
    .map((line) => line.trim())
    .where((line) => line.isNotEmpty)
    .toList();

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
  const SmartCachePageContent({
    required this.cache,
    this.policy,
    this.offlineFallbackError,
  });

  final Map<String, dynamic>? policy;
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

final smartCachePolicyProvider = FutureProvider.autoDispose
    .family<Map<String, dynamic>, String>(
      (ref, roomId) => ref.watch(smartCacheGetProvider)(
        '/v1/rooms/$roomId/smart-cache-policy',
      ),
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
        final results = await Future.wait([
          ref.watch(smartCachePolicyProvider(roomId).future),
          ref.watch(smartCacheFilesProvider(roomId).future),
        ]);
        final cache = results[1];
        final files = smartCacheFilesFromPayload(cache);
        await displayCache.replaceSmartCacheFiles(roomId, files);
        return SmartCachePageContent(policy: results[0], cache: cache);
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

  Future<void> setEnabled(Map<String, dynamic> policy, bool enabled) async {
    if (enabled) {
      await configurePolicy(policy, enableAfterSave: true);
      return;
    }
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('스마트 캐시 끄기'),
        content: const Text('진행 중인 업로드를 취소하고 이 방의 서버 캐시 object 삭제를 예약합니다.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('유지'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('끄기'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await savePolicy(policy, enabled: false);
  }

  Future<void> savePolicy(
    Map<String, dynamic> policy, {
    required bool enabled,
  }) async {
    try {
      await ref
          .read(apiClientProvider)
          .patch('/v1/rooms/${widget.roomId}/smart-cache-policy', {
            'enabled': enabled,
            'quotaBytes': policy['quotaBytes'],
            'maxFileBytes': policy['maxFileBytes'],
            'excludedPatterns': policy['excludedPatterns'] ?? [],
            'pinnedPatterns': policy['pinnedPatterns'] ?? [],
          });
      setState(reload);
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('스마트 캐시 설정 실패: $error')));
      }
    }
  }

  Future<void> configurePolicy(
    Map<String, dynamic> policy, {
    bool? enableAfterSave,
  }) async {
    const megabyte = 1024 * 1024;
    final quota = TextEditingController(
      text: '${((policy['quotaBytes'] as num) / megabyte).round()}',
    );
    final maxFile = TextEditingController(
      text: '${((policy['maxFileBytes'] as num) / megabyte).round()}',
    );
    final excluded = TextEditingController(
      text: (policy['excludedPatterns'] as List<dynamic>? ?? const []).join(
        '\n',
      ),
    );
    final pinned = TextEditingController(
      text: (policy['pinnedPatterns'] as List<dynamic>? ?? const []).join('\n'),
    );
    final formKey = GlobalKey<FormState>();
    var consent = enableAfterSave != true || policy['enabled'] == true;
    final result = await showDialog<Map<String, dynamic>>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setDialogState) => AlertDialog(
          title: const Text('스마트 캐시 범위 설정'),
          content: SizedBox(
            width: 480,
            child: Form(
              key: formKey,
              child: SingleChildScrollView(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Text(
                      '전체 폴더를 동기화하지 않습니다. PC가 제출한 후보 중 quota 안에서 서버가 승인한 파일 원본만 private object storage에 보관합니다.',
                    ),
                    const SizedBox(height: 16),
                    TextFormField(
                      controller: quota,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: '방 전체 한도 (MB)',
                        border: OutlineInputBorder(),
                      ),
                      validator: _positiveInteger,
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: maxFile,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: '파일 하나의 최대 크기 (MB)',
                        border: OutlineInputBorder(),
                      ),
                      validator: _positiveInteger,
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: excluded,
                      minLines: 3,
                      maxLines: 6,
                      decoration: const InputDecoration(
                        labelText: '제외 패턴 (한 줄에 하나)',
                        hintText: 'private/**\n*.tmp',
                        border: OutlineInputBorder(),
                      ),
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: pinned,
                      minLines: 3,
                      maxLines: 6,
                      decoration: const InputDecoration(
                        labelText: '고정 패턴 (한 줄에 하나)',
                        hintText: 'important/**\n*.presentation.pdf',
                        border: OutlineInputBorder(),
                      ),
                    ),
                    if (enableAfterSave == true && policy['enabled'] != true)
                      CheckboxListTile(
                        contentPadding: EdgeInsets.zero,
                        value: consent,
                        onChanged: (value) =>
                            setDialogState(() => consent = value == true),
                        title: const Text('서버 보관 범위와 삭제 정책을 확인했습니다.'),
                      ),
                  ],
                ),
              ),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('취소'),
            ),
            FilledButton(
              onPressed: !consent
                  ? null
                  : () {
                      if (!formKey.currentState!.validate()) return;
                      try {
                        Navigator.pop(
                          context,
                          parseSmartCachePolicyInput(
                            quotaMegabytes: quota.text,
                            maxFileMegabytes: maxFile.text,
                            excludedPatterns: excluded.text,
                            pinnedPatterns: pinned.text,
                            enabled:
                                enableAfterSave ?? (policy['enabled'] == true),
                          ),
                        );
                      } on FormatException catch (error) {
                        ScaffoldMessenger.of(
                          context,
                        ).showSnackBar(SnackBar(content: Text(error.message)));
                      }
                    },
              child: const Text('저장'),
            ),
          ],
        ),
      ),
    );
    quota.dispose();
    maxFile.dispose();
    excluded.dispose();
    pinned.dispose();
    if (result == null) return;
    await savePolicy(result, enabled: result['enabled'] == true);
  }

  static String? _positiveInteger(String? value) {
    final parsed = int.tryParse(value ?? '');
    return parsed == null || parsed <= 0 ? '1 이상의 정수를 입력하세요' : null;
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

  Future<void> remove(Map<String, dynamic> file) async {
    try {
      await ref
          .read(apiClientProvider)
          .delete('/v1/cached-files/${file['id']}');
      if (mounted) setState(reload);
    } catch (error) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('캐시 제거 실패: $error')));
      }
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: const Text('오프라인 스마트 캐시')),
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
        final policy = data.policy;
        final cache = data.cache;
        final files = smartCacheFilesFromPayload(cache);
        return ListView(
          padding: const EdgeInsets.all(16),
          children: [
            if (data.isOfflineFallback)
              SmartCacheOfflineFallbackWarning(
                error: data.offlineFallbackError,
              ),
            if (policy != null) ...[
              SwitchListTile(
                title: const Text('스마트 캐시 사용'),
                subtitle: Text('방 용량 한도: ${policy['quotaBytes']} bytes'),
                value: policy['enabled'] == true,
                onChanged: (value) => setEnabled(policy, value),
              ),
              ListTile(
                leading: const Icon(Icons.tune),
                title: const Text('용량·제외 범위 설정'),
                subtitle: Text(
                  '파일 한도: ${policy['maxFileBytes']} bytes · '
                  '제외 패턴 ${(policy['excludedPatterns'] as List<dynamic>? ?? const []).length}개 · '
                  '고정 패턴 ${(policy['pinnedPatterns'] as List<dynamic>? ?? const []).length}개',
                ),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => configurePolicy(policy),
              ),
            ],
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
                  onRemove: () => remove(file),
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
    required this.onRemove,
  });
  final Map<String, dynamic> file;
  final bool isDownloading;
  final double? progress;
  final VoidCallback onDownload;
  final VoidCallback onRemove;

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
      trailing: PopupMenuButton<String>(
        enabled: !isDownloading,
        onSelected: (value) {
          if (value == 'download') onDownload();
          if (value == 'remove') onRemove();
        },
        itemBuilder: (_) => const [
          PopupMenuItem(value: 'download', child: Text('다운로드')),
          PopupMenuItem(value: 'remove', child: Text('캐시 제거')),
        ],
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
