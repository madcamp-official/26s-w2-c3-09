import 'dart:convert';
import 'package:drift/drift.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../features/auth/auth_controller.dart';
import 'app_database.dart';

final displayCacheProvider = Provider<DisplayCache>((ref) {
  final uid = ref.watch(authControllerProvider).asData?.value?.uid;
  if (uid == null) throw StateError('UNAUTHENTICATED');
  return DisplayCache(ref.watch(appDatabaseProvider), uid);
});

class DisplayCache {
  DisplayCache(this._database, this._ownerUid);
  final AppDatabase _database;
  final String _ownerUid;

  Future<void> replaceDevices(List<Map<String, dynamic>> values) {
    return _replaceTopLevelDevices(_ownerUid, values);
  }

  Future<void> _replaceTopLevelDevices(
    String ownerUid,
    List<Map<String, dynamic>> values,
  ) async {
    await _database.transaction(() async {
      await (_database.delete(
        _database.cachedDevices,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      for (final value in values) {
        await _database
            .into(_database.cachedDevices)
            .insert(
              CachedDevicesCompanion.insert(
                ownerUid: ownerUid,
                id: value['id'] as String,
                payloadJson: jsonEncode(value),
                updatedAt: DateTime.now(),
              ),
            );
      }
    });
  }

  Future<List<Map<String, dynamic>>> devices() async {
    final rows = await (_database.select(
      _database.cachedDevices,
    )..where((row) => row.ownerUid.equals(_ownerUid))).get();
    return rows.map((row) => _decode(row.payloadJson)).toList();
  }

  Future<void> replaceRooms(List<Map<String, dynamic>> values) async {
    final ownerUid = _ownerUid;
    await _database.transaction(() async {
      await (_database.delete(
        _database.cachedRooms,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      for (final value in values) {
        await _database
            .into(_database.cachedRooms)
            .insert(
              CachedRoomsCompanion.insert(
                ownerUid: ownerUid,
                id: value['id'] as String,
                payloadJson: jsonEncode(value),
                updatedAt: DateTime.now(),
              ),
            );
      }
    });
  }

  Future<List<Map<String, dynamic>>> rooms() async {
    final rows = await (_database.select(
      _database.cachedRooms,
    )..where((row) => row.ownerUid.equals(_ownerUid))).get();
    return rows.map((row) => _decode(row.payloadJson)).toList();
  }

  Future<void> replaceCommands(
    String roomId,
    List<Map<String, dynamic>> values,
  ) async {
    final ownerUid = _ownerUid;
    await _database.transaction(() async {
      await (_database.delete(_database.cachedCommands)..where(
            (row) => row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
          ))
          .go();
      for (final value in values) {
        await _database
            .into(_database.cachedCommands)
            .insert(
              CachedCommandsCompanion.insert(
                ownerUid: ownerUid,
                roomId: roomId,
                id: value['id'] as String,
                payloadJson: jsonEncode(value),
                updatedAt: DateTime.now(),
              ),
            );
      }
    });
  }

  Future<List<Map<String, dynamic>>> commands(String roomId) async {
    final rows =
        await (_database.select(_database.cachedCommands)..where(
              (row) =>
                  row.ownerUid.equals(_ownerUid) & row.roomId.equals(roomId),
            ))
            .get();
    return rows.map((row) => _decode(row.payloadJson)).toList();
  }

  Future<void> replaceProposals(
    String roomId,
    List<Map<String, dynamic>> values,
  ) async {
    final ownerUid = _ownerUid;
    await _database.transaction(() async {
      await (_database.delete(_database.cachedProposals)..where(
            (row) => row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
          ))
          .go();
      for (final value in values) {
        await _database
            .into(_database.cachedProposals)
            .insert(
              CachedProposalsCompanion.insert(
                ownerUid: ownerUid,
                roomId: roomId,
                id: value['id'] as String,
                payloadJson: jsonEncode(value),
                updatedAt: DateTime.now(),
              ),
            );
      }
    });
  }

  Future<List<Map<String, dynamic>>> proposals(String roomId) async {
    final rows =
        await (_database.select(_database.cachedProposals)..where(
              (row) =>
                  row.ownerUid.equals(_ownerUid) & row.roomId.equals(roomId),
            ))
            .get();
    return rows.map((row) => _decode(row.payloadJson)).toList();
  }

  Future<void> replaceExecutions(
    String roomId,
    List<Map<String, dynamic>> values,
  ) async {
    final ownerUid = _ownerUid;
    await _database.transaction(() async {
      await (_database.delete(_database.cachedExecutions)..where(
            (row) => row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
          ))
          .go();
      for (final value in values) {
        final execution = Map<String, dynamic>.from(value['execution'] as Map);
        await _database
            .into(_database.cachedExecutions)
            .insert(
              CachedExecutionsCompanion.insert(
                ownerUid: ownerUid,
                roomId: roomId,
                id: execution['id'] as String,
                payloadJson: jsonEncode(value),
                updatedAt: DateTime.now(),
              ),
            );
      }
    });
  }

  Future<List<Map<String, dynamic>>> executions(String roomId) async {
    final rows =
        await (_database.select(_database.cachedExecutions)..where(
              (row) =>
                  row.ownerUid.equals(_ownerUid) & row.roomId.equals(roomId),
            ))
            .get();
    return rows.map((row) => _decode(row.payloadJson)).toList();
  }

  Future<void> saveSnapshot(String roomId, Map<String, dynamic>? value) async {
    final ownerUid = _ownerUid;
    if (value == null) {
      await (_database.delete(_database.cachedRoomSnapshots)..where(
            (row) => row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
          ))
          .go();
      return;
    }
    await _database
        .into(_database.cachedRoomSnapshots)
        .insertOnConflictUpdate(
          CachedRoomSnapshotsCompanion.insert(
            ownerUid: ownerUid,
            roomId: roomId,
            payloadJson: jsonEncode(value),
            updatedAt: DateTime.now(),
          ),
        );
  }

  Future<Map<String, dynamic>?> snapshot(String roomId) async {
    final row =
        await (_database.select(_database.cachedRoomSnapshots)..where(
              (row) =>
                  row.ownerUid.equals(_ownerUid) & row.roomId.equals(roomId),
            ))
            .getSingleOrNull();
    return row == null ? null : _decode(row.payloadJson);
  }

  Map<String, dynamic> _decode(String value) {
    return Map<String, dynamic>.from(jsonDecode(value) as Map);
  }
}
