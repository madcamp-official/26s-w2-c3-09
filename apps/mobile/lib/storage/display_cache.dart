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

  /// Applies an authoritative connection snapshot and removes every display
  /// cache that belonged to rooms no longer returned by the server.
  Future<void> replaceConnectionState({
    required List<Map<String, dynamic>> devices,
    required List<Map<String, dynamic>> rooms,
  }) async {
    final ownerUid = _ownerUid;
    await _database.transaction(() async {
      final previousRooms = await (_database.select(
        _database.cachedRooms,
      )..where((row) => row.ownerUid.equals(ownerUid))).get();
      final activeRoomIds = rooms
          .map((room) => room['id'])
          .whereType<String>()
          .toSet();
      for (final previous in previousRooms) {
        if (!activeRoomIds.contains(previous.id)) {
          await _deleteRoomRows(ownerUid, previous.id);
        }
      }

      await (_database.delete(
        _database.cachedDevices,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      for (final value in devices) {
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

      await (_database.delete(
        _database.cachedRooms,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      for (final value in rooms) {
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

  /// Adds home-only fields to rows that the authoritative pairing gate has
  /// already cached. Missing rows are never inserted, so a late summary
  /// response cannot resurrect a revoked device or removed room.
  Future<void> enrichConnectionState({
    required List<Map<String, dynamic>> devices,
    required List<Map<String, dynamic>> rooms,
  }) async {
    final ownerUid = _ownerUid;
    await _database.transaction(() async {
      final now = DateTime.now();
      for (final value in devices) {
        final id = value['id'];
        if (id is! String) continue;
        final existing =
            await (_database.select(_database.cachedDevices)..where(
                  (row) => row.ownerUid.equals(ownerUid) & row.id.equals(id),
                ))
                .getSingleOrNull();
        if (existing == null) continue;
        final merged = {..._decode(existing.payloadJson), ...value};
        await (_database.update(_database.cachedDevices)..where(
              (row) => row.ownerUid.equals(ownerUid) & row.id.equals(id),
            ))
            .write(
              CachedDevicesCompanion(
                payloadJson: Value(jsonEncode(merged)),
                updatedAt: Value(now),
              ),
            );
      }
      for (final value in rooms) {
        final id = value['id'];
        if (id is! String) continue;
        final existing =
            await (_database.select(_database.cachedRooms)..where(
                  (row) => row.ownerUid.equals(ownerUid) & row.id.equals(id),
                ))
                .getSingleOrNull();
        if (existing == null) continue;
        final merged = {..._decode(existing.payloadJson), ...value};
        await (_database.update(_database.cachedRooms)..where(
              (row) => row.ownerUid.equals(ownerUid) & row.id.equals(id),
            ))
            .write(
              CachedRoomsCompanion(
                payloadJson: Value(jsonEncode(merged)),
                updatedAt: Value(now),
              ),
            );
      }
    });
  }

  Future<void> removeRoomCascade(String roomId) {
    return _database.transaction(() => _deleteRoomRows(_ownerUid, roomId));
  }

  Future<void> removeDeviceCascade(String deviceId) async {
    final ownerUid = _ownerUid;
    await _database.transaction(() async {
      final roomRows = await (_database.select(
        _database.cachedRooms,
      )..where((row) => row.ownerUid.equals(ownerUid))).get();
      for (final roomRow in roomRows) {
        final payload = _decode(roomRow.payloadJson);
        if (payload['desktopDeviceId'] == deviceId) {
          await _deleteRoomRows(ownerUid, roomRow.id);
        }
      }
      await (_database.delete(_database.cachedDevices)..where(
            (row) => row.ownerUid.equals(ownerUid) & row.id.equals(deviceId),
          ))
          .go();
    });
  }

  Future<void> purgeConnectionDisplayData() async {
    final ownerUid = _ownerUid;
    await _database.transaction(() async {
      await (_database.delete(
        _database.cachedDevices,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      await (_database.delete(
        _database.cachedRooms,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      await (_database.delete(
        _database.cachedCommands,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      await (_database.delete(
        _database.cachedProposals,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      await (_database.delete(
        _database.cachedExecutions,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      await (_database.delete(
        _database.cachedRoomSnapshots,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
      // A queued room mutation must not cross an unpaired -> re-paired
      // boundary, even if the new pairing happens to reuse an identifier.
      await (_database.delete(
        _database.mutationOutbox,
      )..where((row) => row.ownerUid.equals(ownerUid))).go();
    });
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
      final previousRooms = await (_database.select(
        _database.cachedRooms,
      )..where((row) => row.ownerUid.equals(ownerUid))).get();
      final replacementIds = values
          .map((room) => room['id'])
          .whereType<String>()
          .toSet();
      for (final previous in previousRooms) {
        if (!replacementIds.contains(previous.id)) {
          await _deleteRoomRows(ownerUid, previous.id);
        }
      }
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
      if (!await _cachedRoomExists(ownerUid, roomId)) return;
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
      if (!await _cachedRoomExists(ownerUid, roomId)) return;
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
      if (!await _cachedRoomExists(ownerUid, roomId)) return;
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
    await _database.transaction(() async {
      if (value == null) {
        await (_database.delete(_database.cachedRoomSnapshots)..where(
              (row) =>
                  row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
            ))
            .go();
        return;
      }
      if (!await _cachedRoomExists(ownerUid, roomId)) return;
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
    });
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

  Future<void> _deleteRoomRows(String ownerUid, String roomId) async {
    await (_database.delete(_database.cachedRooms)..where(
          (row) => row.ownerUid.equals(ownerUid) & row.id.equals(roomId),
        ))
        .go();
    await (_database.delete(_database.cachedCommands)..where(
          (row) => row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
        ))
        .go();
    await (_database.delete(_database.cachedProposals)..where(
          (row) => row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
        ))
        .go();
    await (_database.delete(_database.cachedExecutions)..where(
          (row) => row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
        ))
        .go();
    await (_database.delete(_database.cachedRoomSnapshots)..where(
          (row) => row.ownerUid.equals(ownerUid) & row.roomId.equals(roomId),
        ))
        .go();
    await _deleteRoomOutboxRows(ownerUid, roomId);
  }

  Future<bool> _cachedRoomExists(String ownerUid, String roomId) async {
    final room =
        await (_database.select(_database.cachedRooms)..where(
              (row) => row.ownerUid.equals(ownerUid) & row.id.equals(roomId),
            ))
            .getSingleOrNull();
    return room != null;
  }

  Future<void> _deleteRoomOutboxRows(String ownerUid, String roomId) async {
    final rows = await (_database.select(
      _database.mutationOutbox,
    )..where((row) => row.ownerUid.equals(ownerUid))).get();
    final targetedIds = rows
        .where((row) => _outboxPayloadTargetsRoom(row.payloadJson, roomId))
        .map((row) => row.id)
        .toList();
    if (targetedIds.isEmpty) return;
    await (_database.delete(_database.mutationOutbox)..where(
          (row) => row.ownerUid.equals(ownerUid) & row.id.isIn(targetedIds),
        ))
        .go();
  }

  bool _outboxPayloadTargetsRoom(String payloadJson, String roomId) {
    try {
      final payload = jsonDecode(payloadJson);
      return _containsExactRoomId(payload, roomId) ||
          _containsRoomPath(payload, roomId);
    } on FormatException {
      // Never use substring matching on malformed JSON: it could delete an
      // unrelated mutation whose free-form content happens to mention a room.
      return false;
    }
  }

  bool _containsExactRoomId(Object? value, String roomId) {
    if (value is Map) {
      for (final entry in value.entries) {
        if (entry.key == 'roomId' && entry.value == roomId) return true;
        if (_containsExactRoomId(entry.value, roomId)) return true;
      }
    } else if (value is List) {
      return value.any((item) => _containsExactRoomId(item, roomId));
    }
    return false;
  }

  bool _containsRoomPath(Object? value, String roomId) {
    if (value is Map) {
      for (final entry in value.entries) {
        if (entry.key == 'path' &&
            entry.value is String &&
            _pathTargetsRoom(entry.value as String, roomId)) {
          return true;
        }
        if (_containsRoomPath(entry.value, roomId)) return true;
      }
    } else if (value is List) {
      return value.any((item) => _containsRoomPath(item, roomId));
    }
    return false;
  }

  bool _pathTargetsRoom(String path, String roomId) {
    try {
      final segments = Uri.parse(path).pathSegments;
      for (var index = 0; index + 1 < segments.length; index += 1) {
        if (segments[index] == 'rooms' && segments[index + 1] == roomId) {
          return true;
        }
      }
    } on FormatException {
      return false;
    }
    return false;
  }

  Map<String, dynamic> _decode(String value) {
    return Map<String, dynamic>.from(jsonDecode(value) as Map);
  }
}
