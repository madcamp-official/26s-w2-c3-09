import 'dart:convert';

import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/storage/app_database.dart';
import 'package:mousekeeper/storage/display_cache.dart';

void main() {
  late AppDatabase database;

  setUp(() {
    database = AppDatabase.forTesting(NativeDatabase.memory());
  });

  tearDown(() => database.close());

  test('표시 캐시는 Firebase 사용자 UID별로 격리된다', () async {
    final userA = DisplayCache(database, 'firebase-user-a');
    final userB = DisplayCache(database, 'firebase-user-b');

    await userA.replaceRooms([
      {'id': 'room-a', 'name': 'A room'},
    ]);
    await userB.replaceRooms([
      {'id': 'room-b', 'name': 'B room'},
    ]);

    expect((await userA.rooms()).single['id'], 'room-a');
    expect((await userB.rooms()).single['id'], 'room-b');
  });

  test('방별 명령 캐시는 다른 방과 섞이지 않는다', () async {
    final cache = DisplayCache(database, 'firebase-user-a');
    await cache.replaceRooms([
      {'id': 'room-a'},
      {'id': 'room-b'},
    ]);

    await cache.replaceCommands('room-a', [
      {'id': 'command-a', 'roomId': 'room-a'},
    ]);
    await cache.replaceCommands('room-b', [
      {'id': 'command-b', 'roomId': 'room-b'},
    ]);

    expect((await cache.commands('room-a')).single['id'], 'command-a');
    expect((await cache.commands('room-b')).single['id'], 'command-b');
  });

  test('room 제거는 모든 room-scoped 표시 캐시를 함께 삭제한다', () async {
    final cache = DisplayCache(database, 'firebase-user-a');
    await cache.replaceRooms([
      {'id': 'room-a', 'name': 'A room'},
    ]);
    await cache.replaceCommands('room-a', [
      {'id': 'command-a', 'roomId': 'room-a'},
    ]);
    await cache.replaceProposals('room-a', [
      {'id': 'proposal-a', 'roomId': 'room-a'},
    ]);
    await cache.replaceExecutions('room-a', [
      {
        'execution': {'id': 'execution-a'},
      },
    ]);
    await cache.saveSnapshot('room-a', {'score': 70});

    await cache.removeRoomCascade('room-a');

    expect(await cache.rooms(), isEmpty);
    expect(await cache.commands('room-a'), isEmpty);
    expect(await cache.proposals('room-a'), isEmpty);
    expect(await cache.executions('room-a'), isEmpty);
    expect(await cache.snapshot('room-a'), isNull);
  });

  test('device 제거는 연결된 room과 하위 캐시만 함께 삭제한다', () async {
    final cache = DisplayCache(database, 'firebase-user-a');
    await cache.replaceDevices([
      {'id': 'device-a'},
      {'id': 'device-b'},
    ]);
    await cache.replaceRooms([
      {'id': 'room-a', 'desktopDeviceId': 'device-a'},
      {'id': 'room-b', 'desktopDeviceId': 'device-b'},
    ]);
    await cache.replaceCommands('room-a', [
      {'id': 'command-a'},
    ]);
    await cache.replaceCommands('room-b', [
      {'id': 'command-b'},
    ]);

    await cache.removeDeviceCascade('device-a');

    expect((await cache.devices()).map((item) => item['id']), ['device-b']);
    expect((await cache.rooms()).map((item) => item['id']), ['room-b']);
    expect(await cache.commands('room-a'), isEmpty);
    expect((await cache.commands('room-b')).single['id'], 'command-b');
  });

  test('unpaired purge는 orphan room cache까지 남기지 않는다', () async {
    final cache = DisplayCache(database, 'firebase-user-a');
    await cache.replaceDevices([
      {'id': 'device-a'},
    ]);
    await cache.replaceCommands('orphan-room', [
      {'id': 'command-a'},
    ]);
    await cache.saveSnapshot('orphan-room', {'score': 40});

    await cache.purgeConnectionDisplayData();

    expect(await cache.devices(), isEmpty);
    expect(await cache.commands('orphan-room'), isEmpty);
    expect(await cache.snapshot('orphan-room'), isNull);
  });

  test(
    'removed room cannot be resurrected by a late detail response',
    () async {
      final cache = DisplayCache(database, 'firebase-user-a');
      await cache.replaceRooms([
        {'id': 'room-a'},
      ]);
      await cache.removeRoomCascade('room-a');

      await cache.replaceCommands('room-a', [
        {'id': 'late-command'},
      ]);
      await cache.replaceProposals('room-a', [
        {'id': 'late-proposal'},
      ]);
      await cache.saveSnapshot('room-a', {'score': 99});

      expect(await cache.commands('room-a'), isEmpty);
      expect(await cache.proposals('room-a'), isEmpty);
      expect(await cache.snapshot('room-a'), isNull);
    },
  );

  test(
    'late home summary enrichment cannot resurrect lifecycle rows',
    () async {
      final cache = DisplayCache(database, 'firebase-user-a');
      await cache.replaceConnectionState(
        devices: [
          {'id': 'device-a', 'status': 'ACTIVE'},
        ],
        rooms: [
          {'id': 'room-a', 'desktopDeviceId': 'device-a', 'status': 'ACTIVE'},
        ],
      );
      await cache.removeDeviceCascade('device-a');

      await cache.enrichConnectionState(
        devices: [
          {'id': 'device-a', 'status': 'ACTIVE', 'presence': 'ONLINE_IDLE'},
        ],
        rooms: [
          {
            'id': 'room-a',
            'desktopDeviceId': 'device-a',
            'status': 'ACTIVE',
            'cleanlinessScore': 99,
          },
        ],
      );

      expect(await cache.devices(), isEmpty);
      expect(await cache.rooms(), isEmpty);
    },
  );

  test('home summary enrichment remains owner scoped', () async {
    final userA = DisplayCache(database, 'firebase-user-a');
    final userB = DisplayCache(database, 'firebase-user-b');
    await userA.replaceDevices([
      {'id': 'shared-device', 'status': 'ACTIVE'},
    ]);
    await userB.replaceDevices([
      {'id': 'shared-device', 'status': 'ACTIVE'},
    ]);

    await userA.enrichConnectionState(
      devices: [
        {'id': 'shared-device', 'presence': 'ONLINE_EXECUTING'},
      ],
      rooms: const [],
    );

    expect((await userA.devices()).single['presence'], 'ONLINE_EXECUTING');
    expect((await userB.devices()).single.containsKey('presence'), isFalse);
  });

  test(
    'room cascade removes only outbox mutations targeting that room',
    () async {
      final cache = DisplayCache(database, 'firebase-user-a');
      await _insertOutbox(
        database,
        id: 'nested-room-id',
        ownerUid: 'firebase-user-a',
        payloadJson: jsonEncode({
          'body': {'roomId': 'room-a'},
        }),
      );
      await _insertOutbox(
        database,
        id: 'room-path',
        ownerUid: 'firebase-user-a',
        payloadJson: jsonEncode({'path': '/v1/rooms/room-a/commands'}),
      );
      await _insertOutbox(
        database,
        id: 'other-room',
        ownerUid: 'firebase-user-a',
        payloadJson: jsonEncode({
          'body': {'roomId': 'room-b'},
        }),
      );
      await _insertOutbox(
        database,
        id: 'similar-room-path',
        ownerUid: 'firebase-user-a',
        payloadJson: jsonEncode({'path': '/v1/rooms/room-aa/commands'}),
      );
      await _insertOutbox(
        database,
        id: 'malformed-payload',
        ownerUid: 'firebase-user-a',
        payloadJson: '{"roomId":"room-a"',
      );
      await _insertOutbox(
        database,
        id: 'other-owner',
        ownerUid: 'firebase-user-b',
        payloadJson: jsonEncode({'roomId': 'room-a'}),
      );

      await cache.removeRoomCascade('room-a');

      final remaining = await database.select(database.mutationOutbox).get();
      expect(remaining.map((row) => row.id).toSet(), {
        'other-room',
        'similar-room-path',
        'malformed-payload',
        'other-owner',
      });
    },
  );

  test('unpaired purge removes the current user outbox only', () async {
    final cache = DisplayCache(database, 'firebase-user-a');
    await _insertOutbox(
      database,
      id: 'current-user-pending',
      ownerUid: 'firebase-user-a',
      payloadJson: jsonEncode({'roomId': 'room-a'}),
    );
    await _insertOutbox(
      database,
      id: 'current-user-other-room',
      ownerUid: 'firebase-user-a',
      payloadJson: jsonEncode({'roomId': 'room-b'}),
    );
    await _insertOutbox(
      database,
      id: 'other-user-pending',
      ownerUid: 'firebase-user-b',
      payloadJson: jsonEncode({'roomId': 'room-a'}),
    );

    await cache.purgeConnectionDisplayData();

    final remaining = await database.select(database.mutationOutbox).get();
    expect(remaining.map((row) => row.id), ['other-user-pending']);
  });
}

Future<void> _insertOutbox(
  AppDatabase database, {
  required String id,
  required String ownerUid,
  required String payloadJson,
}) {
  final now = DateTime.now();
  return database
      .into(database.mutationOutbox)
      .insert(
        MutationOutboxCompanion.insert(
          ownerUid: ownerUid,
          id: id,
          mutationType: 'TEST_MUTATION',
          payloadJson: payloadJson,
          nextRetryAt: now,
          createdAt: now,
        ),
      );
}
