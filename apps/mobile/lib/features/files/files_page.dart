import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import '../../core/files/verified_download.dart';
import '../../core/network/api_client.dart';

bool shouldClearBrowseEntries({required bool append}) => !append;

List<Map<String, dynamic>> mergeBrowseEntries({
  required List<Map<String, dynamic>> existing,
  required List<Map<String, dynamic>> received,
  required bool append,
}) => append ? [...existing, ...received] : [...received];

String fileOperationErrorCode(Object error) {
  if (error is DioException) {
    final data = error.response?.data;
    if (data is Map && data['code'] is String) return data['code'] as String;
  }
  final raw = error.toString();
  for (final code in const [
    'DEVICE_OFFLINE',
    'TIMED_OUT',
    'CURSOR_INVALIDATED',
    'SOURCE_NOT_FOUND',
    'SOURCE_CHANGED',
    'OUTSIDE_MANAGED_ROOT',
    'SIZE_LIMIT_EXCEEDED',
    'CHECKSUM_MISMATCH',
    'CANCELLED',
  ]) {
    if (raw.contains(code)) return code;
  }
  return 'FILE_OPERATION_FAILED';
}

String fileOperationErrorMessage(Object error) =>
    switch (fileOperationErrorCode(error)) {
      'DEVICE_OFFLINE' => 'PC 에이전트와 연결되지 않았습니다. 이전에 받은 목록은 그대로 유지됩니다.',
      'TIMED_OUT' => 'PC 응답 시간이 초과되었습니다. 이전에 받은 목록은 그대로 유지됩니다.',
      'CURSOR_INVALIDATED' => '폴더 내용이 바뀌어 다음 페이지를 이어갈 수 없습니다. 첫 페이지를 다시 불러오세요.',
      'SOURCE_NOT_FOUND' => 'PC에서 원본 파일을 찾을 수 없습니다.',
      'SOURCE_CHANGED' => '요청 뒤 원본 파일이 변경되어 가져오기를 중단했습니다.',
      'OUTSIDE_MANAGED_ROOT' => '관리 폴더 밖의 파일 요청이 차단되었습니다.',
      'SIZE_LIMIT_EXCEEDED' => '파일이 전송 크기 제한을 초과했습니다.',
      'CHECKSUM_MISMATCH' => 'checksum이 일치하지 않아 다운로드 파일을 삭제했습니다.',
      'CANCELLED' => '파일 가져오기를 취소했습니다.',
      _ => '파일 요청을 완료하지 못했습니다.',
    };

class FilesPage extends ConsumerStatefulWidget {
  const FilesPage({super.key, required this.roomId});
  final String roomId;

  @override
  ConsumerState<FilesPage> createState() => _FilesPageState();
}

class _FilesPageState extends ConsumerState<FilesPage> {
  final List<Map<String, dynamic>> _entries = [];
  String _relativeDirectory = '';
  String? _nextCursor;
  String? _generation;
  Object? _error;
  bool _loading = false;
  String? _activeTransferId;
  double? _downloadProgress;
  CancelToken? _downloadCancelToken;

  Future<void> _browse({String? cursor, bool append = false}) async {
    setState(() {
      _loading = true;
      _error = null;
      if (shouldClearBrowseEntries(append: append)) {
        _entries.clear();
        _nextCursor = null;
        _generation = null;
      }
    });
    try {
      final api = ref.read(apiClientProvider);
      final created = await api.post(
        '/v1/rooms/${widget.roomId}/file-browse-requests',
        {'relativeDirectory': _relativeDirectory, 'cursor': cursor},
      );
      final completed = await _pollBrowse(created['id'] as String);
      if (completed['status'] != 'READY') {
        throw StateError(
          completed['failureCode'] as String? ?? 'BROWSE_FAILED',
        );
      }
      final page = Map<String, dynamic>.from(
        completed['resultPage'] as Map? ?? const {},
      );
      final received = (page['entries'] as List<dynamic>? ?? const [])
          .map((entry) => Map<String, dynamic>.from(entry as Map))
          .toList();
      if (!mounted) return;
      setState(() {
        final merged = mergeBrowseEntries(
          existing: _entries,
          received: received,
          append: append,
        );
        _entries
          ..clear()
          ..addAll(merged);
        _nextCursor = page['nextCursor'] as String?;
        _generation = completed['desktopGeneration'] as String?;
      });
    } catch (error) {
      if (mounted) setState(() => _error = error);
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<Map<String, dynamic>> _pollBrowse(String requestId) async {
    for (var attempt = 0; attempt < 31; attempt++) {
      final value = await ref
          .read(apiClientProvider)
          .get('/v1/file-browse-requests/$requestId');
      if (value['status'] == 'READY' || value['status'] == 'FAILED') {
        return value;
      }
      await Future<void>.delayed(const Duration(seconds: 2));
    }
    throw TimeoutException('TIMED_OUT');
  }

  Future<void> _openDirectory(String relativePath) async {
    _relativeDirectory = relativePath;
    await _browse();
  }

  Future<void> _goUp() async {
    if (_relativeDirectory.isEmpty) return;
    final normalized = _relativeDirectory.replaceAll('\\', '/');
    final index = normalized.lastIndexOf('/');
    _relativeDirectory = index < 0 ? '' : normalized.substring(0, index);
    await _browse();
  }

  Future<void> _download(Map<String, dynamic> entry) async {
    setState(() {
      _loading = true;
      _error = null;
      _downloadProgress = null;
    });
    try {
      final api = ref.read(apiClientProvider);
      final transfer = await api.post(
        '/v1/rooms/${widget.roomId}/file-transfers',
        {'sourceRelativePath': entry['relativePath']},
        idempotencyKey: const Uuid().v4(),
      );
      final id = transfer['id'] as String;
      setState(() => _activeTransferId = id);
      Map<String, dynamic> state = transfer;
      for (
        var attempt = 0;
        attempt < 300 && state['status'] != 'READY';
        attempt++
      ) {
        if (['FAILED', 'EXPIRED', 'CANCELLED'].contains(state['status'])) {
          throw StateError(
            state['failureCode'] as String? ??
                state['status'] as String? ??
                'TRANSFER_FAILED',
          );
        }
        await Future<void>.delayed(const Duration(seconds: 2));
        state = await api.get('/v1/file-transfers/$id');
      }
      if (state['status'] != 'READY') throw TimeoutException('TIMED_OUT');
      final target = await api.get('/v1/file-transfers/$id/download');
      _downloadCancelToken = CancelToken();
      final destination = await VerifiedDownload.save(
        api: api,
        url: target['downloadUrl'] as String,
        expectedSha256: target['sha256'] as String,
        fileName: entry['relativePath'] as String,
        onProgress: (received, total) {
          if (mounted && total > 0) {
            setState(() => _downloadProgress = received / total);
          }
        },
        cancelToken: _downloadCancelToken,
      );
      await api.post('/v1/file-transfers/$id/ack', const {});
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('checksum 확인 후 저장 완료: ${destination.path}')),
        );
      }
    } catch (error) {
      if (mounted) setState(() => _error = error);
    } finally {
      if (mounted) {
        setState(() {
          _loading = false;
          _activeTransferId = null;
          _downloadProgress = null;
          _downloadCancelToken = null;
        });
      }
    }
  }

  Future<void> _cancelTransfer() async {
    final id = _activeTransferId;
    if (id == null) return;
    try {
      _downloadCancelToken?.cancel('User cancelled transfer');
      await ref.read(apiClientProvider).delete('/v1/file-transfers/$id');
      if (mounted) {
        setState(() {
          _activeTransferId = null;
          _loading = false;
          _downloadProgress = null;
          _error = StateError('CANCELLED');
        });
      }
    } catch (error) {
      if (mounted) setState(() => _error = error);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('온라인 파일')),
      body: Column(
        children: [
          if (_loading) LinearProgressIndicator(value: _downloadProgress),
          if (_relativeDirectory.isNotEmpty || _generation != null)
            ListTile(
              leading: IconButton(
                tooltip: '상위 폴더',
                onPressed: _loading ? null : _goUp,
                icon: const Icon(Icons.arrow_upward),
              ),
              title: Text(
                _relativeDirectory.isEmpty ? '관리 폴더' : _relativeDirectory,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              subtitle: _generation == null
                  ? null
                  : Text('목록 버전: $_generation'),
            ),
          if (_error != null)
            MaterialBanner(
              content: Text(fileOperationErrorMessage(_error!)),
              leading: const Icon(Icons.cloud_off_outlined),
              actions: [
                TextButton(
                  onPressed: _loading ? null : () => _browse(),
                  child: const Text('다시 시도'),
                ),
              ],
            ),
          if (_activeTransferId != null)
            ListTile(
              leading: const Icon(Icons.downloading_outlined),
              title: const Text('파일을 안전하게 가져오는 중'),
              subtitle: Text(
                _downloadProgress == null
                    ? 'PC 응답 또는 업로드 대기 중'
                    : '${(_downloadProgress! * 100).round()}% 다운로드됨',
              ),
              trailing: TextButton(
                onPressed: _cancelTransfer,
                child: const Text('취소'),
              ),
            ),
          Expanded(child: _content()),
        ],
      ),
    );
  }

  Widget _content() {
    if (_entries.isEmpty && _error == null && !_loading) {
      return Center(
        child: FilledButton.icon(
          onPressed: _browse,
          icon: const Icon(Icons.folder_open),
          label: const Text('PC 파일 조회'),
        ),
      );
    }
    if (_entries.isEmpty && _loading) {
      return const Center(child: Text('PC의 파일 목록을 기다리는 중입니다.'));
    }
    if (_entries.isEmpty) {
      return const Center(child: Text('이 폴더에 표시할 파일이 없습니다.'));
    }
    return RefreshIndicator(
      onRefresh: _browse,
      child: ListView(
        children: [
          ..._entries.map((entry) {
            final isFile = entry['type'] == 'FILE';
            return ListTile(
              leading: Icon(
                isFile
                    ? Icons.insert_drive_file_outlined
                    : Icons.folder_outlined,
              ),
              title: Text(entry['name'] as String),
              subtitle: Text(entry['relativePath'] as String),
              trailing: isFile
                  ? IconButton(
                      onPressed: _loading ? null : () => _download(entry),
                      icon: const Icon(Icons.download),
                    )
                  : const Icon(Icons.chevron_right),
              onTap: isFile || _loading
                  ? null
                  : () => _openDirectory(entry['relativePath'] as String),
            );
          }),
          if (_nextCursor != null)
            Padding(
              padding: const EdgeInsets.all(16),
              child: OutlinedButton(
                onPressed: _loading
                    ? null
                    : () => _browse(cursor: _nextCursor, append: true),
                child: const Text('다음 파일 불러오기'),
              ),
            ),
        ],
      ),
    );
  }
}
