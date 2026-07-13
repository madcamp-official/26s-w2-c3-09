import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import '../../core/files/verified_download.dart';
import '../../core/network/api_client.dart';
import '../auth/connection_gate_controller.dart';

// Shared v1.4 contract names the current-directory scope explicitly.
const fileSearchScopeDirectory = 'CURRENT_DIRECTORY';
const fileSearchScopeManagedRoot = 'MANAGED_ROOT';
const fileSearchQueryMinLength = 2;
const fileSearchQueryMaxLength = 100;

int fileSearchQueryLength(String value) => value.trim().runes.length;

bool isValidFileSearchQuery(String value) {
  final length = fileSearchQueryLength(value);
  return length >= fileSearchQueryMinLength &&
      length <= fileSearchQueryMaxLength;
}

abstract interface class FileBrowseGateway {
  Future<Map<String, dynamic>> createRequest(
    String roomId,
    Map<String, dynamic> body,
  );

  Future<Map<String, dynamic>> getRequest(String requestId);
}

class ApiFileBrowseGateway implements FileBrowseGateway {
  ApiFileBrowseGateway(this._api);

  final ApiClient _api;

  @override
  Future<Map<String, dynamic>> createRequest(
    String roomId,
    Map<String, dynamic> body,
  ) => _api.post('/v1/rooms/$roomId/file-browse-requests', body);

  @override
  Future<Map<String, dynamic>> getRequest(String requestId) =>
      _api.get('/v1/file-browse-requests/$requestId');
}

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
    'ROOM_REMOVED',
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
      'ROOM_REMOVED' => '연결 해제된 폴더입니다. 파일 화면을 닫아 주세요.',
      _ => '파일 요청을 완료하지 못했습니다.',
    };

String formatFileSize(Object? rawBytes) {
  if (rawBytes is! num) return '폴더';
  final bytes = rawBytes.toDouble();
  if (bytes < 1024) return '${bytes.toInt()} B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  if (bytes < 1024 * 1024 * 1024) {
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
  return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
}

String formatFileModifiedAt(Object? value) {
  if (value is! String) return '수정 시각 없음';
  final parsed = DateTime.tryParse(value)?.toLocal();
  if (parsed == null) return '수정 시각 확인 불가';
  String two(int number) => number.toString().padLeft(2, '0');
  return '${parsed.year}.${two(parsed.month)}.${two(parsed.day)} '
      '${two(parsed.hour)}:${two(parsed.minute)}';
}

class FilesPage extends ConsumerStatefulWidget {
  const FilesPage({
    super.key,
    required this.roomId,
    this.roomName,
    this.browseGateway,
    this.enforceConnectionGuard = true,
  });
  final String roomId;
  final String? roomName;
  final FileBrowseGateway? browseGateway;
  final bool enforceConnectionGuard;

  @override
  ConsumerState<FilesPage> createState() => _FilesPageState();
}

class _FilesPageState extends ConsumerState<FilesPage> {
  final List<Map<String, dynamic>> _entries = [];
  final TextEditingController _searchController = TextEditingController();
  String _relativeDirectory = '';
  String? _nextCursor;
  String? _generation;
  Object? _error;
  bool _browseLoading = false;
  bool _transferLoading = false;
  String? _activeTransferId;
  double? _downloadProgress;
  CancelToken? _downloadCancelToken;
  Timer? _searchDebounce;
  String _searchQuery = '';
  String _searchScope = fileSearchScopeDirectory;
  int _requestVersion = 0;
  bool _disposed = false;
  bool _transferCancelled = false;

  FileBrowseGateway get _browseGateway =>
      widget.browseGateway ?? ApiFileBrowseGateway(ref.read(apiClientProvider));

  bool get _searchActive => isValidFileSearchQuery(_searchQuery);
  bool get _busy => _browseLoading || _transferLoading;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(_browse());
    });
  }

  @override
  void dispose() {
    _disposed = true;
    _transferCancelled = true;
    _requestVersion++;
    _searchDebounce?.cancel();
    _downloadCancelToken?.cancel('Files page disposed');
    final transferId = _activeTransferId;
    if (transferId != null) {
      unawaited(
        ref
            .read(apiClientProvider)
            .delete('/v1/file-transfers/$transferId')
            .then<void>((_) {}, onError: (_, _) {}),
      );
    }
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _browse({String? cursor, bool append = false}) async {
    final requestVersion = ++_requestVersion;
    final query = _searchActive ? _searchQuery.trim() : null;
    final relativeDirectory = _relativeDirectory;
    final searchScope = _searchScope;
    setState(() {
      _browseLoading = true;
      _error = null;
      if (shouldClearBrowseEntries(append: append)) {
        _entries.clear();
        _nextCursor = null;
        _generation = null;
      }
    });
    try {
      final body = <String, dynamic>{
        'relativeDirectory': relativeDirectory,
        'cursor': cursor,
      };
      if (query != null) {
        body['query'] = query;
        body['searchScope'] = searchScope;
      }
      final created = await _browseGateway.createRequest(widget.roomId, body);
      final completed = await _pollBrowse(
        created['id'] as String,
        requestVersion,
      );
      if (requestVersion != _requestVersion) return;
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
      if (!mounted || requestVersion != _requestVersion) return;
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
      if (cursor != null &&
          fileOperationErrorCode(error) == 'CURSOR_INVALIDATED' &&
          mounted &&
          requestVersion == _requestVersion) {
        // The cursor is coupled to the desktop index generation. Drop every
        // stale page and restart this same directory/query/scope exactly once.
        await _browse();
        return;
      }
      if (error is! _StaleBrowseResponse &&
          mounted &&
          requestVersion == _requestVersion) {
        setState(() => _error = error);
      }
    } finally {
      if (mounted && requestVersion == _requestVersion) {
        setState(() => _browseLoading = false);
      }
    }
  }

  Future<Map<String, dynamic>> _pollBrowse(
    String requestId,
    int requestVersion,
  ) async {
    for (var attempt = 0; attempt < 31; attempt++) {
      if (requestVersion != _requestVersion) {
        throw const _StaleBrowseResponse();
      }
      final value = await _browseGateway.getRequest(requestId);
      if (requestVersion != _requestVersion) {
        throw const _StaleBrowseResponse();
      }
      if (value['status'] == 'READY' || value['status'] == 'FAILED') {
        return value;
      }
      await Future<void>.delayed(const Duration(seconds: 2));
    }
    throw TimeoutException('TIMED_OUT');
  }

  Future<void> _openDirectory(String relativePath) async {
    _searchDebounce?.cancel();
    _searchController.clear();
    _searchQuery = '';
    _relativeDirectory = relativePath;
    await _browse();
  }

  Future<void> _openBreadcrumb(String relativeDirectory) async {
    _searchDebounce?.cancel();
    _searchController.clear();
    _searchQuery = '';
    _relativeDirectory = relativeDirectory;
    await _browse();
  }

  void _onSearchChanged(String value) {
    _searchDebounce?.cancel();
    _requestVersion++;
    final query = value.trim();
    setState(() {
      _searchQuery = query;
      _browseLoading = false;
      _error = null;
      _entries.clear();
      _nextCursor = null;
      _generation = null;
    });
    if (query.isEmpty) {
      unawaited(_browse());
      return;
    }
    if (!isValidFileSearchQuery(query)) return;
    _searchDebounce = Timer(const Duration(milliseconds: 300), () {
      if (mounted && _searchQuery.trim() == query) unawaited(_browse());
    });
  }

  void _changeSearchScope(String? value) {
    if (value == null || value == _searchScope) return;
    _searchDebounce?.cancel();
    _requestVersion++;
    setState(() {
      _searchScope = value;
      _entries.clear();
      _nextCursor = null;
      _error = null;
      _browseLoading = false;
    });
    if (_searchActive) unawaited(_browse());
  }

  Future<void> _download(Map<String, dynamic> entry) async {
    _transferCancelled = false;
    setState(() {
      _transferLoading = true;
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
      if (_disposed || _transferCancelled || !mounted) {
        try {
          await api.delete('/v1/file-transfers/$id');
        } finally {
          throw StateError('CANCELLED');
        }
      }
      setState(() => _activeTransferId = id);
      Map<String, dynamic> state = transfer;
      for (
        var attempt = 0;
        attempt < 300 && state['status'] != 'READY';
        attempt++
      ) {
        _throwIfTransferStopped();
        if (['FAILED', 'EXPIRED', 'CANCELLED'].contains(state['status'])) {
          throw StateError(
            state['failureCode'] as String? ??
                state['status'] as String? ??
                'TRANSFER_FAILED',
          );
        }
        await Future<void>.delayed(const Duration(seconds: 2));
        _throwIfTransferStopped();
        state = await api.get('/v1/file-transfers/$id');
        _throwIfTransferStopped();
      }
      _throwIfTransferStopped();
      if (state['status'] != 'READY') throw TimeoutException('TIMED_OUT');
      final target = await api.get('/v1/file-transfers/$id/download');
      _throwIfTransferStopped();
      _downloadCancelToken = CancelToken();
      final destination = await saveVerifiedDownloadAndAck(
        cancelToken: _downloadCancelToken,
        save: () => VerifiedDownload.save(
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
        ),
        acknowledge: () async {
          await api.post('/v1/file-transfers/$id/ack', const {});
        },
      );
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
          _transferLoading = false;
          _activeTransferId = null;
          _downloadProgress = null;
          _downloadCancelToken = null;
          _transferCancelled = false;
        });
      }
    }
  }

  Future<void> _cancelTransfer() async {
    final id = _activeTransferId;
    if (id == null || _transferCancelled) return;
    _transferCancelled = true;
    _downloadCancelToken?.cancel('User cancelled transfer');
    try {
      await ref.read(apiClientProvider).delete('/v1/file-transfers/$id');
      if (mounted) {
        setState(() => _error = StateError('CANCELLED'));
      }
    } catch (error) {
      _transferCancelled = false;
      if (mounted) setState(() => _error = error);
    }
  }

  void _throwIfTransferStopped() {
    if (_disposed || _transferCancelled || !mounted) {
      throw StateError('CANCELLED');
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.enforceConnectionGuard) {
      ref.listen(connectionGateControllerProvider, (previous, next) {
        final gate = next.asData?.value;
        if (gate == null ||
            gate.rooms.any((room) => room['id'] == widget.roomId)) {
          return;
        }
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) Navigator.of(context).popUntil((route) => route.isFirst);
        });
      });
    }
    return Scaffold(
      appBar: AppBar(title: Text('${widget.roomName ?? '연결된 폴더'} 파일')),
      body: Column(
        children: [
          if (_busy) LinearProgressIndicator(value: _downloadProgress),
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 12, 12, 4),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: TextField(
                    key: const ValueKey('file-search-field'),
                    controller: _searchController,
                    onChanged: _onSearchChanged,
                    textInputAction: TextInputAction.search,
                    decoration: InputDecoration(
                      labelText: '파일·폴더 이름 검색',
                      hintText: '2자 이상 입력',
                      prefixIcon: const Icon(Icons.search),
                      suffixIcon: _searchQuery.isEmpty
                          ? null
                          : IconButton(
                              tooltip: '검색 지우기',
                              onPressed: () {
                                _searchController.clear();
                                _onSearchChanged('');
                              },
                              icon: const Icon(Icons.clear),
                            ),
                      border: const OutlineInputBorder(),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                SizedBox(
                  width: 142,
                  child: DropdownButtonFormField<String>(
                    key: const ValueKey('file-search-scope'),
                    initialValue: _searchScope,
                    decoration: const InputDecoration(
                      labelText: '검색 범위',
                      border: OutlineInputBorder(),
                    ),
                    items: const [
                      DropdownMenuItem(
                        value: fileSearchScopeDirectory,
                        child: Text('현재 폴더'),
                      ),
                      DropdownMenuItem(
                        value: fileSearchScopeManagedRoot,
                        child: Text('전체 폴더'),
                      ),
                    ],
                    onChanged: _browseLoading ? null : _changeSearchScope,
                  ),
                ),
              ],
            ),
          ),
          if (_searchQuery.isNotEmpty && !_searchActive)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
              child: Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  fileSearchQueryLength(_searchQuery) < fileSearchQueryMinLength
                      ? '검색어를 2자 이상 입력해 주세요.'
                      : '검색어를 100자 이하로 입력해 주세요.',
                ),
              ),
            ),
          FileBreadcrumb(
            relativeDirectory: _relativeDirectory,
            enabled: !_busy,
            onSelected: _openBreadcrumb,
          ),
          if (_generation != null)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  '목록 버전: $_generation',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ),
            ),
          if (_error != null)
            MaterialBanner(
              content: Text(fileOperationErrorMessage(_error!)),
              leading: const Icon(Icons.cloud_off_outlined),
              actions: [
                TextButton(
                  onPressed: _busy
                      ? null
                      : fileOperationErrorCode(_error!) == 'ROOM_REMOVED'
                      ? () => Navigator.of(
                          context,
                        ).popUntil((route) => route.isFirst)
                      : () => _browse(),
                  child: Text(
                    fileOperationErrorCode(_error!) == 'ROOM_REMOVED'
                        ? '닫기'
                        : '다시 시도',
                  ),
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
                onPressed: _transferCancelled ? null : _cancelTransfer,
                child: const Text('취소'),
              ),
            ),
          Expanded(child: _content()),
        ],
      ),
    );
  }

  Widget _content() {
    if (_searchQuery.isNotEmpty && !_searchActive) {
      return Center(
        child: Text(
          fileSearchQueryLength(_searchQuery) < fileSearchQueryMinLength
              ? '2자 이상 입력하면 이름 검색을 시작합니다.'
              : '검색어는 100자 이하로 입력해 주세요.',
        ),
      );
    }
    if (_entries.isEmpty && _browseLoading) {
      return const Center(child: Text('PC의 파일 목록을 기다리는 중입니다.'));
    }
    if (_entries.isEmpty) {
      return Center(
        child: Text(_searchActive ? '검색 결과가 없습니다.' : '이 폴더에 표시할 파일이 없습니다.'),
      );
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
              subtitle: Text(
                '${_searchActive ? '${entry['relativePath']}\n' : ''}'
                '${formatFileSize(entry['sizeBytes'])} · '
                '${formatFileModifiedAt(entry['modifiedAt'])}',
              ),
              trailing: isFile
                  ? IconButton(
                      onPressed: _busy ? null : () => _download(entry),
                      icon: const Icon(Icons.download),
                    )
                  : const Icon(Icons.chevron_right),
              onTap: isFile || _busy
                  ? null
                  : () => _openDirectory(entry['relativePath'] as String),
            );
          }),
          if (_nextCursor != null)
            Padding(
              padding: const EdgeInsets.all(16),
              child: OutlinedButton(
                onPressed: _busy
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

class FileBreadcrumb extends StatelessWidget {
  const FileBreadcrumb({
    super.key,
    required this.relativeDirectory,
    required this.enabled,
    required this.onSelected,
  });

  final String relativeDirectory;
  final bool enabled;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    final segments = relativeDirectory
        .replaceAll('\\', '/')
        .split('/')
        .where((segment) => segment.isNotEmpty)
        .toList(growable: false);
    final widgets = <Widget>[
      TextButton.icon(
        onPressed: enabled ? () => onSelected('') : null,
        icon: const Icon(Icons.home_outlined, size: 18),
        label: const Text('관리 폴더'),
      ),
    ];
    var current = '';
    for (final segment in segments) {
      current = current.isEmpty ? segment : '$current/$segment';
      final target = current;
      widgets
        ..add(const Icon(Icons.chevron_right, size: 18))
        ..add(
          TextButton(
            onPressed: enabled ? () => onSelected(target) : null,
            child: Text(segment),
          ),
        );
    }
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: Row(children: widgets),
    );
  }
}

class _StaleBrowseResponse implements Exception {
  const _StaleBrowseResponse();
}
