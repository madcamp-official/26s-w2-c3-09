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

    await cache.replaceCommands('room-a', [
      {'id': 'command-a', 'roomId': 'room-a'},
    ]);
    await cache.replaceCommands('room-b', [
      {'id': 'command-b', 'roomId': 'room-b'},
    ]);

    expect((await cache.commands('room-a')).single['id'], 'command-a');
    expect((await cache.commands('room-b')).single['id'], 'command-b');
  });
}
