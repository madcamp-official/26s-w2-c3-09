import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import '../../core/files/verified_download.dart';
import '../../core/network/api_client.dart';
import '../../core/sync/realtime_controller.dart';
import '../../core/widgets/cheese_loading.dart';
import '../auth/connection_gate_controller.dart';

const _pixelInk = Color(0xFF30251F);
const _pixelPaper = Color(0xFFFFF4D6);
const _pixelCanvas = Color(0xFFE7CFA9);

// Shared v1.4 contract names the current-directory scope explicitly.
const fileSearchScopeDirectory = 'CURRENT_DIRECTORY';
const fileSearchScopeManagedRoot = 'MANAGED_ROOT';
const fileSearchQueryMinLength = 2;
const fileSearchQueryMaxLength = 100;
const fileBrowseStatusFallbackInterval = Duration(seconds: 5);

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

final fileBrowseGatewayProvider = Provider<FileBrowseGateway>((ref) {
  return ApiFileBrowseGateway(ref.watch(apiClientProvider));
});

class FileDirectoryBrowseRequest {
  const FileDirectoryBrowseRequest({
    required this.roomId,
    required this.relativeDirectory,
    this.cursor,
    this.query,
    this.searchScope,
  });

  final String roomId;
  final String relativeDirectory;
  final String? cursor;
  final String? query;
  final String? searchScope;

  Map<String, dynamic> toBody() {
    final body = <String, dynamic>{
      'relativeDirectory': relativeDirectory,
      'cursor': cursor,
    };
    final trimmedQuery = query?.trim();
    if (trimmedQuery != null && trimmedQuery.isNotEmpty) {
      body['query'] = trimmedQuery;
      body['searchScope'] = searchScope ?? fileSearchScopeDirectory;
    }
    return body;
  }
}

typedef FileDirectoryBrowseRequester =
    Future<Map<String, dynamic>> Function(FileDirectoryBrowseRequest request);

typedef FileBrowseStatusFetcher =
    Future<Map<String, dynamic>> Function(String requestId);

final fileDirectoryBrowseRequesterProvider =
    Provider<FileDirectoryBrowseRequester>((ref) {
      final gateway = ref.watch(fileBrowseGatewayProvider);
      return (request) =>
          gateway.createRequest(request.roomId, request.toBody());
    });

final fileBrowseStatusFetcherProvider = Provider<FileBrowseStatusFetcher>((
  ref,
) {
  final gateway = ref.watch(fileBrowseGatewayProvider);
  return gateway.getRequest;
});

bool shouldClearBrowseEntries({required bool append}) => !append;

List<Map<String, dynamic>> mergeBrowseEntries({
  required List<Map<String, dynamic>> existing,
  required List<Map<String, dynamic>> received,
  required bool append,
}) => append ? [...existing, ...received] : [...received];

class FileDirectoryState {
  FileDirectoryState({
    required List<Map<String, dynamic>> entries,
    required this.nextCursor,
    required this.generation,
    this.isStale = false,
  }) : entries = List.unmodifiable(
         entries.map((entry) => Map<String, dynamic>.unmodifiable(entry)),
       );

  const FileDirectoryState.empty()
    : entries = const [],
      nextCursor = null,
      generation = null,
      isStale = false;

  final List<Map<String, dynamic>> entries;
  final String? nextCursor;
  final String? generation;
  final bool isStale;

  bool get isEmpty => entries.isEmpty;

  FileDirectoryState withPage({
    required List<Map<String, dynamic>> received,
    required bool append,
    required String? nextCursor,
    required String? generation,
  }) {
    final nextEntries = mergeBrowseEntries(
      existing: entries,
      received: received,
      append: append,
    );
    if (!isStale &&
        this.nextCursor == nextCursor &&
        this.generation == generation &&
        _entryListsShallowEquals(entries, nextEntries)) {
      return this;
    }
    return FileDirectoryState(
      entries: nextEntries,
      nextCursor: nextCursor,
      generation: generation,
    );
  }

  FileDirectoryState applyUpdate({
    required String currentRelativeDirectory,
    required FileDirectoryUpdate update,
  }) {
    final normalizedCurrent = normalizeFileRelativePath(
      currentRelativeDirectory,
    );
    final entry = update.entry;
    final entryPath = entry == null ? null : _entryRelativePath(entry);
    final sourcePath = update.previousRelativePath ?? update.relativePath;
    final sourceParent = sourcePath == null
        ? null
        : parentRelativeDirectoryOf(sourcePath);
    final destinationParent = entryPath == null
        ? null
        : parentRelativeDirectoryOf(entryPath);

    return switch (update.kind) {
      FileDirectoryUpdateKind.added => _addOrMarkStale(
        normalizedCurrent,
        update.parentRelativePath ?? destinationParent,
        entry,
      ),
      FileDirectoryUpdateKind.updated => _replaceIfVisible(
        normalizedCurrent,
        update.parentRelativePath ?? destinationParent,
        entry,
      ),
      FileDirectoryUpdateKind.removed => _removeIfVisible(
        normalizedCurrent,
        update.parentRelativePath ?? sourceParent,
        sourcePath ?? entryPath,
      ),
      FileDirectoryUpdateKind.moved => _moveIfRelevant(
        normalizedCurrent: normalizedCurrent,
        sourceParent: sourceParent,
        sourcePath: sourcePath,
        destinationParent: update.parentRelativePath ?? destinationParent,
        entry: entry,
      ),
    };
  }

  FileDirectoryState _addOrMarkStale(
    String currentRelativeDirectory,
    String? parentRelativePath,
    Map<String, dynamic>? entry,
  ) {
    if (entry == null ||
        normalizeFileRelativePath(parentRelativePath ?? '') !=
            currentRelativeDirectory) {
      return this;
    }
    final path = _entryRelativePath(entry);
    if (path == null) return this;
    final existingIndex = _entryIndex(path);
    if (existingIndex >= 0) return _replaceAt(existingIndex, entry);
    if (nextCursor != null) return markStale();
    return _withSortedEntries([...entries, entry]);
  }

  FileDirectoryState _replaceIfVisible(
    String currentRelativeDirectory,
    String? parentRelativePath,
    Map<String, dynamic>? entry,
  ) {
    if (entry == null ||
        normalizeFileRelativePath(parentRelativePath ?? '') !=
            currentRelativeDirectory) {
      return this;
    }
    final path = _entryRelativePath(entry);
    if (path == null) return this;
    final existingIndex = _entryIndex(path);
    if (existingIndex < 0) return this;
    return _replaceAt(existingIndex, entry);
  }

  FileDirectoryState _removeIfVisible(
    String currentRelativeDirectory,
    String? parentRelativePath,
    String? relativePath,
  ) {
    if (relativePath == null ||
        normalizeFileRelativePath(parentRelativePath ?? '') !=
            currentRelativeDirectory) {
      return this;
    }
    final existingIndex = _entryIndex(relativePath);
    if (existingIndex < 0) return this;
    final next = [...entries]..removeAt(existingIndex);
    return FileDirectoryState(
      entries: next,
      nextCursor: nextCursor,
      generation: generation,
      isStale: isStale || nextCursor != null,
    );
  }

  FileDirectoryState _moveIfRelevant({
    required String normalizedCurrent,
    required String? sourceParent,
    required String? sourcePath,
    required String? destinationParent,
    required Map<String, dynamic>? entry,
  }) {
    final sourceInCurrent =
        sourcePath != null &&
        normalizeFileRelativePath(sourceParent ?? '') == normalizedCurrent;
    final destinationInCurrent =
        entry != null &&
        normalizeFileRelativePath(destinationParent ?? '') == normalizedCurrent;
    if (!sourceInCurrent && !destinationInCurrent) return this;

    var nextEntries = [...entries];
    var changed = false;
    if (sourcePath != null) {
      final existingIndex = _entryIndex(sourcePath, nextEntries);
      if (existingIndex >= 0) {
        nextEntries.removeAt(existingIndex);
        changed = true;
      }
    }
    if (destinationInCurrent) {
      final destinationPath = _entryRelativePath(entry);
      if (destinationPath == null) return this;
      final existingIndex = _entryIndex(destinationPath, nextEntries);
      if (existingIndex >= 0) {
        final existing = nextEntries[existingIndex];
        if (!_mapShallowEquals(existing, entry)) {
          nextEntries[existingIndex] = entry;
          changed = true;
        }
      } else if (nextCursor == null) {
        nextEntries.add(entry);
        changed = true;
      } else {
        return changed
            ? FileDirectoryState(
                entries: nextEntries,
                nextCursor: nextCursor,
                generation: generation,
                isStale: true,
              )
            : markStale();
      }
    }
    if (!changed) return this;
    nextEntries = sortFileDirectoryEntries(nextEntries);
    return FileDirectoryState(
      entries: nextEntries,
      nextCursor: nextCursor,
      generation: generation,
      isStale: isStale || nextCursor != null,
    );
  }

  FileDirectoryState markStale() => isStale
      ? this
      : FileDirectoryState(
          entries: entries,
          nextCursor: nextCursor,
          generation: generation,
          isStale: true,
        );

  FileDirectoryState _replaceAt(int index, Map<String, dynamic> entry) {
    final existing = entries[index];
    if (_mapShallowEquals(existing, entry)) return this;
    final next = [...entries]..[index] = entry;
    return _withSortedEntries(next);
  }

  FileDirectoryState _withSortedEntries(List<Map<String, dynamic>> next) =>
      FileDirectoryState(
        entries: sortFileDirectoryEntries(next),
        nextCursor: nextCursor,
        generation: generation,
        isStale: isStale || nextCursor != null,
      );

  int _entryIndex(
    String relativePath, [
    List<Map<String, dynamic>>? candidates,
  ]) => (candidates ?? entries).indexWhere(
    (entry) =>
        _entryRelativePath(entry) == normalizeFileRelativePath(relativePath),
  );
}

enum FileDirectoryUpdateKind { added, removed, updated, moved }

class FileDirectoryUpdate {
  const FileDirectoryUpdate({
    required this.kind,
    this.parentRelativePath,
    this.relativePath,
    this.previousRelativePath,
    this.entry,
  });

  final FileDirectoryUpdateKind kind;
  final String? parentRelativePath;
  final String? relativePath;
  final String? previousRelativePath;
  final Map<String, dynamic>? entry;
}

FileDirectoryUpdate? fileDirectoryUpdateFromRealtime(
  RealtimeFileDirectoryUpdate update,
) {
  final kind = switch (update.kind) {
    'FILE_ADDED' => FileDirectoryUpdateKind.added,
    'FILE_REMOVED' => FileDirectoryUpdateKind.removed,
    'FILE_UPDATED' => FileDirectoryUpdateKind.updated,
    'FILE_MOVED' => FileDirectoryUpdateKind.moved,
    _ => null,
  };
  if (kind == null) return null;
  return FileDirectoryUpdate(
    kind: kind,
    parentRelativePath: update.parentRelativePath,
    relativePath: update.relativePath,
    previousRelativePath: update.previousRelativePath,
    entry: update.entry,
  );
}

String normalizeFileRelativePath(String value) => value
    .replaceAll('\\', '/')
    .split('/')
    .where((segment) => segment.isNotEmpty)
    .join('/');

String parentRelativeDirectoryOf(String relativePath) {
  final normalized = normalizeFileRelativePath(relativePath);
  final lastSeparator = normalized.lastIndexOf('/');
  if (lastSeparator < 0) return '';
  return normalized.substring(0, lastSeparator);
}

List<Map<String, dynamic>> sortFileDirectoryEntries(
  Iterable<Map<String, dynamic>> entries,
) {
  final sorted = entries.map(Map<String, dynamic>.from).toList();
  sorted.sort((left, right) {
    final leftIsDirectory = left['type'] == 'DIRECTORY';
    final rightIsDirectory = right['type'] == 'DIRECTORY';
    if (leftIsDirectory != rightIsDirectory) {
      return leftIsDirectory ? -1 : 1;
    }
    final leftName = left['name'];
    final rightName = right['name'];
    return (leftName is String ? leftName : '').compareTo(
      rightName is String ? rightName : '',
    );
  });
  return sorted;
}

String? _entryRelativePath(Map<String, dynamic> entry) {
  final path = entry['relativePath'];
  return path is String ? normalizeFileRelativePath(path) : null;
}

bool _mapShallowEquals(Map<String, dynamic> left, Map<String, dynamic> right) {
  if (left.length != right.length) return false;
  for (final entry in left.entries) {
    if (right[entry.key] != entry.value) return false;
  }
  return true;
}

bool _entryListsShallowEquals(
  List<Map<String, dynamic>> left,
  List<Map<String, dynamic>> right,
) {
  if (left.length != right.length) return false;
  for (var index = 0; index < left.length; index += 1) {
    if (!_mapShallowEquals(left[index], right[index])) return false;
  }
  return true;
}

Map<String, dynamic> patchFileTransferStateForRealtimeUpdate({
  required Map<String, dynamic> current,
  required RealtimeFileTransferUpdate update,
}) {
  if (current['id'] != update.transferId) return current;
  if (current['status'] == update.status &&
      current['failureCode'] == update.failureCode) {
    return current;
  }
  final next = <String, dynamic>{...current, 'status': update.status};
  if (update.failureCode != null) {
    next['failureCode'] = update.failureCode;
  } else {
    next.remove('failureCode');
  }
  return Map<String, dynamic>.unmodifiable(next);
}

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
  FileDirectoryState _directoryState = const FileDirectoryState.empty();
  final TextEditingController _searchController = TextEditingController();
  String _relativeDirectory = '';
  Object? _error;
  bool _browseLoading = false;
  bool _transferLoading = false;
  String? _activeBrowseRequestId;
  RealtimeFileBrowseUpdate? _lastBrowseUpdate;
  Completer<RealtimeFileBrowseUpdate?>? _browseUpdateWaiter;
  String? _activeTransferId;
  String? _activeTransferStatus;
  double? _downloadProgress;
  CancelToken? _downloadCancelToken;
  RealtimeFileTransferUpdate? _lastTransferUpdate;
  Completer<RealtimeFileTransferUpdate?>? _transferUpdateWaiter;
  Timer? _searchDebounce;
  String _searchQuery = '';
  String _searchScope = fileSearchScopeDirectory;
  int _requestVersion = 0;
  bool _disposed = false;
  bool _transferCancelled = false;

  Future<Map<String, dynamic>> _createBrowseRequest(
    FileDirectoryBrowseRequest request,
  ) {
    final gateway = widget.browseGateway;
    if (gateway != null) {
      return gateway.createRequest(request.roomId, request.toBody());
    }
    return ref.read(fileDirectoryBrowseRequesterProvider)(request);
  }

  Future<Map<String, dynamic>> _getBrowseRequest(String requestId) {
    final gateway = widget.browseGateway;
    if (gateway != null) return gateway.getRequest(requestId);
    return ref.read(fileBrowseStatusFetcherProvider)(requestId);
  }

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
    _completeBrowseWaiter();
    _completeTransferWaiter();
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
    _completeBrowseWaiter();
    final query = _searchActive ? _searchQuery.trim() : null;
    final relativeDirectory = _relativeDirectory;
    final searchScope = _searchScope;
    setState(() {
      _browseLoading = true;
      _error = null;
      if (shouldClearBrowseEntries(append: append)) {
        _directoryState = const FileDirectoryState.empty();
      }
    });
    try {
      final created = await _createBrowseRequest(
        FileDirectoryBrowseRequest(
          roomId: widget.roomId,
          relativeDirectory: relativeDirectory,
          cursor: cursor,
          query: query,
          searchScope: searchScope,
        ),
      );
      final requestId = created['id'] as String;
      if (requestVersion != _requestVersion) return;
      _activeBrowseRequestId = requestId;
      final completed = await _waitForBrowseCompletion(created, requestVersion);
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
      final nextDirectoryState = _directoryState.withPage(
        received: received,
        append: append,
        nextCursor: page['nextCursor'] as String?,
        generation: completed['desktopGeneration'] as String?,
      );
      if (!identical(nextDirectoryState, _directoryState)) {
        setState(() {
          _directoryState = nextDirectoryState;
        });
      }
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
        _activeBrowseRequestId = null;
        _lastBrowseUpdate = null;
        setState(() => _browseLoading = false);
      }
    }
  }

  Future<Map<String, dynamic>> _waitForBrowseCompletion(
    Map<String, dynamic> initial,
    int requestVersion,
  ) async {
    var value = initial;
    if (value['status'] == 'READY' || value['status'] == 'FAILED') {
      return value;
    }
    final requestId = value['id'] as String;
    for (var attempt = 0; attempt < 31; attempt++) {
      if (requestVersion != _requestVersion) {
        throw const _StaleBrowseResponse();
      }
      final realtimeUpdate = await _waitForBrowseUpdate(
        requestId,
        requestVersion,
      );
      if (requestVersion != _requestVersion) {
        throw const _StaleBrowseResponse();
      }
      value = await _getBrowseRequest(requestId);
      if (requestVersion != _requestVersion) {
        throw const _StaleBrowseResponse();
      }
      if (value['status'] == 'READY' || value['status'] == 'FAILED') {
        return value;
      }
      if (realtimeUpdate?.status == 'FAILED') {
        throw StateError(realtimeUpdate?.failureCode ?? 'BROWSE_FAILED');
      }
    }
    throw TimeoutException('TIMED_OUT');
  }

  Future<RealtimeFileBrowseUpdate?> _waitForBrowseUpdate(
    String requestId,
    int requestVersion,
  ) async {
    final latest = _lastBrowseUpdate;
    if (latest != null && latest.requestId == requestId) {
      _lastBrowseUpdate = null;
      return latest;
    }
    final completer = Completer<RealtimeFileBrowseUpdate?>();
    _browseUpdateWaiter = completer;
    final timer = Timer(fileBrowseStatusFallbackInterval, () {
      if (!completer.isCompleted) completer.complete(null);
    });
    try {
      return await completer.future;
    } finally {
      timer.cancel();
      if (identical(_browseUpdateWaiter, completer)) {
        _browseUpdateWaiter = null;
      }
      if (requestVersion != _requestVersion) {
        throw const _StaleBrowseResponse();
      }
    }
  }

  void _completeBrowseWaiter() {
    final waiter = _browseUpdateWaiter;
    if (waiter != null && !waiter.isCompleted) waiter.complete(null);
    _browseUpdateWaiter = null;
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
    _completeBrowseWaiter();
    final query = value.trim();
    setState(() {
      _searchQuery = query;
      _browseLoading = false;
      _error = null;
      _directoryState = const FileDirectoryState.empty();
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
    _completeBrowseWaiter();
    setState(() {
      _searchScope = value;
      _directoryState = const FileDirectoryState.empty();
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
      _activeTransferStatus = 'REQUESTED';
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
      setState(() {
        _activeTransferId = id;
        _activeTransferStatus = transfer['status'] as String? ?? 'REQUESTED';
      });
      Map<String, dynamic> state = transfer;
      for (
        var attempt = 0;
        attempt < 40 && state['status'] != 'READY';
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
        final realtimeUpdate = await _waitForTransferUpdate(id);
        _throwIfTransferStopped();
        if (realtimeUpdate == null) {
          state = await api.get('/v1/file-transfers/$id');
        } else {
          state = patchFileTransferStateForRealtimeUpdate(
            current: state,
            update: realtimeUpdate,
          );
        }
        if (mounted) {
          setState(() => _activeTransferStatus = state['status'] as String?);
        }
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
          _activeTransferStatus = null;
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

  Future<RealtimeFileTransferUpdate?> _waitForTransferUpdate(
    String transferId,
  ) async {
    final latest = _lastTransferUpdate;
    if (latest != null && latest.transferId == transferId) {
      _lastTransferUpdate = null;
      return latest;
    }
    final completer = Completer<RealtimeFileTransferUpdate?>();
    _transferUpdateWaiter = completer;
    final timer = Timer(const Duration(seconds: 15), () {
      if (!completer.isCompleted) completer.complete(null);
    });
    try {
      return await completer.future;
    } finally {
      timer.cancel();
      if (identical(_transferUpdateWaiter, completer)) {
        _transferUpdateWaiter = null;
      }
    }
  }

  void _completeTransferWaiter() {
    final waiter = _transferUpdateWaiter;
    if (waiter != null && !waiter.isCompleted) waiter.complete(null);
    _transferUpdateWaiter = null;
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(realtimeFileBrowseUpdateProvider, (previous, next) {
      if (next == null ||
          identical(previous, next) ||
          next.requestId != _activeBrowseRequestId ||
          !mounted) {
        return;
      }
      _lastBrowseUpdate = next;
      final waiter = _browseUpdateWaiter;
      if (waiter != null && !waiter.isCompleted) {
        waiter.complete(next);
      }
    });
    ref.listen(realtimeFileTransferUpdateProvider, (previous, next) {
      if (next == null ||
          identical(previous, next) ||
          next.transferId != _activeTransferId ||
          !mounted) {
        return;
      }
      _lastTransferUpdate = next;
      final waiter = _transferUpdateWaiter;
      if (waiter != null && !waiter.isCompleted) {
        waiter.complete(next);
      }
      setState(() => _activeTransferStatus = next.status);
    });
    ref.listen(realtimeFileDirectoryUpdateProvider, (previous, next) {
      if (next == null ||
          identical(previous, next) ||
          next.roomId != widget.roomId ||
          _searchActive ||
          !mounted) {
        return;
      }
      final update = fileDirectoryUpdateFromRealtime(next);
      if (update == null) return;
      final patched = _directoryState.applyUpdate(
        currentRelativeDirectory: _relativeDirectory,
        update: update,
      );
      if (!identical(patched, _directoryState)) {
        setState(() => _directoryState = patched);
      }
    });
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
      backgroundColor: _pixelCanvas,
      appBar: AppBar(
        backgroundColor: _pixelInk,
        foregroundColor: _pixelPaper,
        title: Text(
          '${widget.roomName ?? '연결된 폴더'} 파일',
          style: const TextStyle(fontWeight: FontWeight.w900),
        ),
      ),
      body: CheeseLoadingOverlay(
        loading: _busy,
        progress: _downloadProgress,
        message: _transferLoading
            ? '파일을 안전하게 가져오는 중입니다'
            : '데스크탑의 폴더 응답을 기다리는 중입니다',
        child: Column(
          children: [
            if (_busy) LinearProgressIndicator(value: _downloadProgress),
            if (_activeTransferId != null && _downloadProgress == null)
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    'Transfer status: ${_activeTransferStatus ?? 'REQUESTED'}',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ),
              ),
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
                        filled: true,
                        fillColor: _pixelPaper,
                        border: const OutlineInputBorder(
                          borderRadius: BorderRadius.zero,
                          borderSide: BorderSide(color: _pixelInk, width: 2),
                        ),
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
                        filled: true,
                        fillColor: _pixelPaper,
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.zero,
                          borderSide: BorderSide(color: _pixelInk, width: 2),
                        ),
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
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 4,
                ),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    fileSearchQueryLength(_searchQuery) <
                            fileSearchQueryMinLength
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
            if (_directoryState.generation != null)
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    '목록 버전: ${_directoryState.generation}',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ),
              ),
            if (_directoryState.isStale)
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 4, 16, 0),
                child: Row(
                  children: [
                    const Icon(Icons.sync_problem_outlined, size: 16),
                    const SizedBox(width: 6),
                    const Expanded(
                      child: Text('파일 목록이 바뀌었을 수 있어요. 필요하면 새로고침하세요.'),
                    ),
                    TextButton(
                      onPressed: _busy ? null : () => _browse(),
                      child: const Text('새로고침'),
                    ),
                  ],
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
    if (_directoryState.isEmpty && _browseLoading) {
      return const Center(child: Text('PC의 파일 목록을 기다리는 중입니다.'));
    }
    if (_directoryState.isEmpty) {
      return Center(
        child: Text(_searchActive ? '검색 결과가 없습니다.' : '이 폴더에 표시할 파일이 없습니다.'),
      );
    }
    return RefreshIndicator(
      onRefresh: _browse,
      child: ListView(
        children: [
          ..._directoryState.entries.map((entry) {
            final isFile = entry['type'] == 'FILE';
            return ListTile(
              tileColor: _pixelPaper,
              shape: const Border(
                bottom: BorderSide(color: _pixelInk, width: 1),
              ),
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
          if (_directoryState.nextCursor != null)
            Padding(
              padding: const EdgeInsets.all(16),
              child: OutlinedButton(
                onPressed: _busy
                    ? null
                    : () => _browse(
                        cursor: _directoryState.nextCursor,
                        append: true,
                      ),
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
