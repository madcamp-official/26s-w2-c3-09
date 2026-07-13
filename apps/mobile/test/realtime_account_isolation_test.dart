import 'dart:async';

import 'package:drift/native.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/models/character_state.dart';
import 'package:mousekeeper/core/sync/realtime_account_session.dart';
import 'package:mousekeeper/core/sync/realtime_controller.dart';
import 'package:mousekeeper/storage/app_database.dart';
import 'package:mousekeeper/storage/display_cache.dart';

final _testOwnerUidProvider =
    NotifierProvider<_TestOwnerUidController, String?>(
      _TestOwnerUidController.new,
    );

class _TestOwnerUidController extends Notifier<String?> {
  @override
  String? build() => 'account-a';

  void setUid(String? value) => state = value;
}

void main() {
  test('account switch and disconnect invalidate captured socket sessions', () {
    final guard = RealtimeAccountSessionGuard();
    expect(guard.bind('account-a'), isTrue);
    final accountA = guard.beginConnection()!;
    expect(guard.isCurrent(accountA, 'account-a'), isTrue);

    expect(guard.bind('account-b'), isTrue);
    expect(guard.isCurrent(accountA, 'account-b'), isFalse);
    final accountB = guard.beginConnection()!;
    expect(guard.isCurrent(accountB, 'account-b'), isTrue);

    guard.invalidate();
    expect(guard.isCurrent(accountB, 'account-b'), isFalse);
  });

  test(
    'late account A replay cannot touch account B cache or cursor',
    () async {
      final database = AppDatabase.forTesting(NativeDatabase.memory());
      addTearDown(database.close);
      final accountA = DisplayCache(database, 'account-a');
      final accountB = DisplayCache(database, 'account-b');
      await accountA.replaceRooms([
        {'id': _roomA, 'status': 'ACTIVE'},
      ]);
      await accountB.replaceRooms([
        {'id': _roomB, 'status': 'ACTIVE'},
      ]);

      final fetchStarted = Completer<void>();
      final fetchResult = Completer<List<Map<String, dynamic>>>();
      var fetchCalls = 0;
      final container = ProviderContainer(
        overrides: [
          appDatabaseProvider.overrideWithValue(database),
          realtimeOwnerUidProvider.overrideWith(
            (ref) => ref.watch(_testOwnerUidProvider),
          ),
          realtimeAutoConnectProvider.overrideWithValue(false),
          realtimeEventFetcherProvider.overrideWithValue((_) {
            fetchCalls++;
            if (fetchCalls > 1) {
              return Future.value([
                {
                  'eventId': _secondEventId,
                  'eventType': 'room.removed',
                  'aggregateId': _roomB,
                  'sequence': 2,
                  'payload': {'roomId': _roomB, 'status': 'REMOVED'},
                },
              ]);
            }
            if (!fetchStarted.isCompleted) fetchStarted.complete();
            return fetchResult.future;
          }),
        ],
      );
      addTearDown(container.dispose);

      final subscription = container.listen(
        realtimeRevisionProvider,
        (_, _) {},
      );
      addTearDown(subscription.close);
      final replay = container
          .read(realtimeRevisionProvider.notifier)
          .replayCurrentAccountForTesting();
      await fetchStarted.future;

      container.read(_testOwnerUidProvider.notifier).setUid('account-b');
      fetchResult.complete([
        {
          'eventId': _eventId,
          'eventType': 'room.removed',
          'aggregateId': _roomA,
          'sequence': 1,
          'payload': {'roomId': _roomA, 'status': 'REMOVED'},
        },
      ]);
      await replay;

      expect((await accountA.rooms()).map((room) => room['id']), [_roomA]);
      expect((await accountB.rooms()).map((room) => room['id']), [_roomB]);
      expect(await database.select(database.syncCursors).get(), isEmpty);

      // The B generation must be able to replay immediately; A's stale socket
      // or replay flag cannot block the new account.
      await container
          .read(realtimeRevisionProvider.notifier)
          .replayCurrentAccountForTesting();
      expect((await accountA.rooms()).map((room) => room['id']), [_roomA]);
      expect(await accountB.rooms(), isEmpty);
      final cursor = await database.select(database.syncCursors).getSingle();
      expect(cursor.ownerUid, 'account-b');
      expect(cursor.lastSequence, 2);
    },
  );

  test('account-bound character and notice providers reset on uid change', () {
    final container = ProviderContainer(
      overrides: [
        realtimeOwnerUidProvider.overrideWith(
          (ref) => ref.watch(_testOwnerUidProvider),
        ),
      ],
    );
    addTearDown(container.dispose);

    container
        .read(realtimeCharacterKindProvider.notifier)
        .emit(CharacterState.success);
    container
        .read(realtimeNoticeProvider.notifier)
        .emit(
          const RealtimeNotice(
            eventId: 'account-a-event',
            eventType: 'execution.updated',
            message: 'done',
          ),
        );
    container
        .read(realtimeHomeUpdateProvider.notifier)
        .emit(
          const RealtimeHomeUpdate(
            kind: RealtimeHomeUpdateKind.presence,
            eventType: 'presence.updated',
            deviceId: 'device-a',
            presence: 'ONLINE_IDLE',
          ),
        );
    expect(
      container.read(realtimeCharacterKindProvider),
      CharacterState.success,
    );
    expect(container.read(realtimeNoticeProvider), isNotNull);
    expect(container.read(realtimeHomeUpdateProvider), isNotNull);

    container.read(_testOwnerUidProvider.notifier).setUid('account-b');

    expect(container.read(realtimeCharacterKindProvider), isNull);
    expect(container.read(realtimeNoticeProvider), isNull);
    expect(container.read(realtimeHomeUpdateProvider), isNull);
  });
}

const _eventId = '018f4c7b-1ad6-7c95-bf34-5e45881f98b1';
const _secondEventId = '018f4c7b-1ad6-7c95-bf34-5e45881f98b4';
const _roomA = '018f4c7b-1ad6-7c95-bf34-5e45881f98b2';
const _roomB = '018f4c7b-1ad6-7c95-bf34-5e45881f98b3';
