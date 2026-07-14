import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/rooms/file_command_page.dart';

void main() {
  test('builds an approval-first rename command', () {
    final command = buildManualFileCommand(
      intent: ManualFileCommandIntent.rename,
      rootId: ' root:downloads ',
      sourceRelativePath: r'reports\old.pdf',
      newName: ' final.pdf ',
    );

    expect(command, {
      'intent': 'RENAME',
      'payload': {
        'rootId': 'root:downloads',
        'sourceRelativePath': 'reports/old.pdf',
        'newName': 'final.pdf',
      },
      'metadata': {'requiresApproval': true},
    });
  });

  test('builds move commands from newline separated source paths', () {
    final command = buildManualFileCommand(
      intent: ManualFileCommandIntent.move,
      rootId: 'root:downloads',
      sourceRelativePathsText:
          'reports/final.pdf\nreports/final.pdf\nnotes.txt',
      destinationRelativeDirectory: ' Archive ',
    );

    expect(command['intent'], 'MOVE');
    expect(command['payload'], {
      'rootId': 'root:downloads',
      'sourceRelativePaths': ['reports/final.pdf', 'notes.txt'],
      'destinationRelativeDirectory': 'Archive',
    });
  });

  test('rejects missing root ids and unsafe paths before submitting', () {
    expect(
      () => buildManualFileCommand(
        intent: ManualFileCommandIntent.trash,
        rootId: '',
        sourceRelativePathsText: 'reports/old.pdf',
      ),
      throwsFormatException,
    );
    expect(
      () => buildManualFileCommand(
        intent: ManualFileCommandIntent.rename,
        rootId: 'root:downloads',
        sourceRelativePath: '../outside.txt',
        newName: 'safe.txt',
      ),
      throwsFormatException,
    );
  });

  testWidgets('submits a rename command from the mobile form', (tester) async {
    Map<String, dynamic>? submitted;
    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(
          home: FileCommandPage(
            roomId: 'room-id',
            rootId: 'root:downloads',
            submit: (command) async {
              submitted = command;
              return false;
            },
          ),
        ),
      ),
    );

    await tester.enterText(
      find.byKey(const ValueKey('manual-file-command-source')),
      'reports/old.pdf',
    );
    await tester.enterText(
      find.byKey(const ValueKey('manual-file-command-new-name')),
      'final.pdf',
    );
    await tester.tap(find.byKey(const ValueKey('manual-file-command-submit')));
    await tester.pumpAndSettle();

    expect(submitted?['intent'], 'RENAME');
    expect(submitted?['payload'], {
      'rootId': 'root:downloads',
      'sourceRelativePath': 'reports/old.pdf',
      'newName': 'final.pdf',
    });
  });
}
