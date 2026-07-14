import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../core/sync/mutation_queue.dart';

typedef FileCommandSubmit = Future<bool> Function(Map<String, dynamic> command);

enum ManualFileCommandIntent {
  rename('RENAME', 'Rename'),
  move('MOVE', 'Move'),
  trash('TRASH', 'Trash'),
  create('CREATE', 'Create');

  const ManualFileCommandIntent(this.wireName, this.label);

  final String wireName;
  final String label;
}

enum ManualCreateKind {
  file('FILE', 'File'),
  directory('DIRECTORY', 'Directory');

  const ManualCreateKind(this.wireName, this.label);

  final String wireName;
  final String label;
}

final _windowsReservedFileNamePattern = RegExp(
  r'^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?$',
  caseSensitive: false,
);

Map<String, dynamic> buildManualFileCommand({
  required ManualFileCommandIntent intent,
  required String rootId,
  String sourceRelativePath = '',
  String sourceRelativePathsText = '',
  String newName = '',
  String destinationRelativeDirectory = '',
  ManualCreateKind createKind = ManualCreateKind.file,
  String createRelativePath = '',
}) {
  final normalizedRootId = rootId.trim();
  if (normalizedRootId.isEmpty) {
    throw const FormatException('MISSING_ROOT_ID');
  }

  Map<String, dynamic> envelope(Map<String, dynamic> payload) => {
    'intent': intent.wireName,
    'payload': payload,
    'metadata': {'requiresApproval': true},
  };

  switch (intent) {
    case ManualFileCommandIntent.rename:
      return envelope({
        'rootId': normalizedRootId,
        'sourceRelativePath': normalizeRelativePathInput(sourceRelativePath),
        'newName': normalizeFileNameInput(newName),
      });
    case ManualFileCommandIntent.move:
      return envelope({
        'rootId': normalizedRootId,
        'sourceRelativePaths': parseRelativePathLines(sourceRelativePathsText),
        'destinationRelativeDirectory': normalizeRelativeDirectoryInput(
          destinationRelativeDirectory,
          allowEmpty: true,
        ),
      });
    case ManualFileCommandIntent.trash:
      return envelope({
        'rootId': normalizedRootId,
        'sourceRelativePaths': parseRelativePathLines(sourceRelativePathsText),
      });
    case ManualFileCommandIntent.create:
      return envelope({
        'rootId': normalizedRootId,
        'kind': createKind.wireName,
        'relativePath': normalizeRelativePathInput(createRelativePath),
      });
  }
}

List<String> parseRelativePathLines(String value) {
  final seen = <String>{};
  final paths = value
      .split('\n')
      .map(normalizeRelativePathInput)
      .where((path) => seen.add(path))
      .toList(growable: false);
  if (paths.isEmpty) throw const FormatException('PATH_REQUIRED');
  if (paths.length > 200) throw const FormatException('TOO_MANY_PATHS');
  return paths;
}

String normalizeRelativePathInput(String value) =>
    normalizeRelativeDirectoryInput(value, allowEmpty: false);

String normalizeRelativeDirectoryInput(
  String value, {
  required bool allowEmpty,
}) {
  final normalized = value.trim().replaceAll('\\', '/');
  if (normalized.isEmpty) {
    if (allowEmpty) return '';
    throw const FormatException('PATH_REQUIRED');
  }
  if (normalized.length > 1024) throw const FormatException('PATH_TOO_LONG');
  if (normalized.startsWith('/') ||
      normalized.startsWith('\\') ||
      RegExp(r'^[A-Za-z]:').hasMatch(normalized) ||
      normalized.contains('\u0000')) {
    throw const FormatException('ONLY_RELATIVE_PATH_ALLOWED');
  }
  final segments = normalized.split('/');
  if (!segments.every(_isSafePathSegment)) {
    throw const FormatException('ONLY_SAFE_PATH_SEGMENTS_ALLOWED');
  }
  return normalized;
}

String normalizeFileNameInput(String value) {
  final normalized = value.trim();
  if (normalized.isEmpty) {
    throw const FormatException('FILE_NAME_REQUIRED');
  }
  if (normalized.length > 255) {
    throw const FormatException('FILE_NAME_TOO_LONG');
  }
  if (normalized.contains('/') ||
      normalized.contains('\\') ||
      normalized.contains('\u0000') ||
      !_isSafePathSegment(normalized)) {
    throw const FormatException('ONLY_SINGLE_SAFE_FILE_NAME_ALLOWED');
  }
  return normalized;
}

bool _isSafePathSegment(String segment) =>
    segment.isNotEmpty &&
    segment != '.' &&
    segment != '..' &&
    !_windowsReservedFileNamePattern.hasMatch(segment);

class FileCommandPage extends ConsumerStatefulWidget {
  const FileCommandPage({
    super.key,
    required this.roomId,
    required this.rootId,
    this.submit,
  });

  final String roomId;
  final String rootId;
  final FileCommandSubmit? submit;

  @override
  ConsumerState<FileCommandPage> createState() => _FileCommandPageState();
}

class _FileCommandPageState extends ConsumerState<FileCommandPage> {
  final _source = TextEditingController();
  final _newName = TextEditingController();
  final _destination = TextEditingController();
  final _createPath = TextEditingController();
  ManualFileCommandIntent _intent = ManualFileCommandIntent.rename;
  ManualCreateKind _createKind = ManualCreateKind.file;
  bool _submitting = false;

  @override
  void dispose() {
    _source.dispose();
    _newName.dispose();
    _destination.dispose();
    _createPath.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final hasRootId = widget.rootId.trim().isNotEmpty;
    return Scaffold(
      appBar: AppBar(title: const Text('File command')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          const Card(
            color: Color(0xFFE3F2FD),
            child: ListTile(
              leading: Icon(Icons.verified_user_outlined),
              title: Text('Approval-first file command'),
              subtitle: Text(
                'This only creates a server command. The desktop agent checks the managed root and creates a proposal before any file is changed.',
              ),
            ),
          ),
          if (!hasRootId) ...[
            const SizedBox(height: 12),
            Card(
              color: Theme.of(context).colorScheme.errorContainer,
              child: const ListTile(
                leading: Icon(Icons.error_outline),
                title: Text('Missing managed-root id'),
                subtitle: Text(
                  'Reconnect this folder from the desktop agent before sending file commands.',
                ),
              ),
            ),
          ],
          const SizedBox(height: 16),
          DropdownButtonFormField<ManualFileCommandIntent>(
            key: const ValueKey('manual-file-command-intent'),
            initialValue: _intent,
            decoration: const InputDecoration(
              labelText: 'Command',
              border: OutlineInputBorder(),
            ),
            items: ManualFileCommandIntent.values
                .map(
                  (intent) => DropdownMenuItem(
                    value: intent,
                    child: Text(intent.label),
                  ),
                )
                .toList(growable: false),
            onChanged: _submitting
                ? null
                : (value) => setState(() => _intent = value ?? _intent),
          ),
          const SizedBox(height: 12),
          if (_intent == ManualFileCommandIntent.rename) ...[
            _pathField(
              controller: _source,
              label: 'Source path',
              hint: 'reports/old.pdf',
              maxLines: 1,
            ),
            const SizedBox(height: 12),
            TextFormField(
              key: const ValueKey('manual-file-command-new-name'),
              controller: _newName,
              enabled: !_submitting,
              decoration: const InputDecoration(
                labelText: 'New name',
                hintText: 'final.pdf',
                border: OutlineInputBorder(),
              ),
            ),
          ] else if (_intent == ManualFileCommandIntent.move) ...[
            _pathField(
              controller: _source,
              label: 'Source paths',
              hint: 'reports/final.pdf\nreports/notes.txt',
              maxLines: 5,
            ),
            const SizedBox(height: 12),
            TextFormField(
              key: const ValueKey('manual-file-command-destination'),
              controller: _destination,
              enabled: !_submitting,
              decoration: const InputDecoration(
                labelText: 'Destination directory',
                hintText: 'Archive or blank for managed-root top',
                border: OutlineInputBorder(),
              ),
            ),
          ] else if (_intent == ManualFileCommandIntent.trash) ...[
            _pathField(
              controller: _source,
              label: 'Paths to trash',
              hint: 'tmp/noise.log\nold/draft.txt',
              maxLines: 5,
            ),
          ] else ...[
            DropdownButtonFormField<ManualCreateKind>(
              initialValue: _createKind,
              decoration: const InputDecoration(
                labelText: 'Create kind',
                border: OutlineInputBorder(),
              ),
              items: ManualCreateKind.values
                  .map(
                    (kind) =>
                        DropdownMenuItem(value: kind, child: Text(kind.label)),
                  )
                  .toList(growable: false),
              onChanged: _submitting
                  ? null
                  : (value) =>
                        setState(() => _createKind = value ?? _createKind),
            ),
            const SizedBox(height: 12),
            TextFormField(
              key: const ValueKey('manual-file-command-create-path'),
              controller: _createPath,
              enabled: !_submitting,
              decoration: const InputDecoration(
                labelText: 'Relative path to create',
                hintText: 'notes/today.md',
                border: OutlineInputBorder(),
              ),
            ),
          ],
          const SizedBox(height: 16),
          FilledButton.icon(
            key: const ValueKey('manual-file-command-submit'),
            onPressed: _submitting || !hasRootId ? null : _submit,
            icon: _submitting
                ? const SizedBox.square(
                    dimension: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.pending_actions_outlined),
            label: const Text('Request proposal'),
          ),
        ],
      ),
    );
  }

  Widget _pathField({
    required TextEditingController controller,
    required String label,
    required String hint,
    required int maxLines,
  }) => TextFormField(
    key: const ValueKey('manual-file-command-source'),
    controller: controller,
    enabled: !_submitting,
    minLines: maxLines == 1 ? 1 : 3,
    maxLines: maxLines,
    decoration: InputDecoration(
      labelText: label,
      hintText: hint,
      border: const OutlineInputBorder(),
    ),
  );

  Future<void> _submit() async {
    late final Map<String, dynamic> command;
    try {
      command = buildManualFileCommand(
        intent: _intent,
        rootId: widget.rootId,
        sourceRelativePath: _source.text,
        sourceRelativePathsText: _source.text,
        newName: _newName.text,
        destinationRelativeDirectory: _destination.text,
        createKind: _createKind,
        createRelativePath: _createPath.text,
      );
    } on FormatException catch (error) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Invalid command input: ${error.message}')),
      );
      return;
    }

    setState(() => _submitting = true);
    try {
      final queued = widget.submit != null
          ? await widget.submit!(command)
          : (await ref
                    .read(mutationQueueProvider)
                    .postOrQueue(
                      mutationType: 'CREATE_COMMAND',
                      path: '/v1/rooms/${widget.roomId}/commands',
                      body: command,
                      idempotencyKey: const Uuid().v4(),
                      roomId: widget.roomId,
                    ))
                .queued;
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            queued
                ? 'Saved offline. It will be sent when the server connection is restored.'
                : 'The desktop agent will prepare a proposal.',
          ),
        ),
      );
      Navigator.pop(context, true);
    } catch (error) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Command request failed: $error')));
      setState(() => _submitting = false);
    }
  }
}
