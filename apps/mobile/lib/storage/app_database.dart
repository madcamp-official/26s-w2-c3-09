import 'dart:io';
import 'package:drift/drift.dart';
import 'package:drift/native.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

part 'app_database.g.dart';

class CachedDevices extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get id => text()();
  TextColumn get payloadJson => text()();
  DateTimeColumn get updatedAt => dateTime()();
  @override
  Set<Column> get primaryKey => {ownerUid, id};
}

class CachedRooms extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get id => text()();
  TextColumn get payloadJson => text()();
  DateTimeColumn get updatedAt => dateTime()();
  @override
  Set<Column> get primaryKey => {ownerUid, id};
}

class CachedCommands extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get roomId => text()();
  TextColumn get id => text()();
  TextColumn get payloadJson => text()();
  DateTimeColumn get updatedAt => dateTime()();
  @override
  Set<Column> get primaryKey => {ownerUid, id};
}

class CachedProposals extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get roomId => text()();
  TextColumn get id => text()();
  TextColumn get payloadJson => text()();
  DateTimeColumn get updatedAt => dateTime()();
  @override
  Set<Column> get primaryKey => {ownerUid, id};
}

class CachedExecutions extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get roomId => text()();
  TextColumn get id => text()();
  TextColumn get payloadJson => text()();
  DateTimeColumn get updatedAt => dateTime()();
  @override
  Set<Column> get primaryKey => {ownerUid, id};
}

class CachedRoomSnapshots extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get roomId => text()();
  TextColumn get payloadJson => text()();
  DateTimeColumn get updatedAt => dateTime()();
  @override
  Set<Column> get primaryKey => {ownerUid, roomId};
}

class MutationOutbox extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get id => text()();
  TextColumn get mutationType => text()();
  TextColumn get payloadJson => text()();
  IntColumn get attemptCount => integer().withDefault(const Constant(0))();
  DateTimeColumn get nextRetryAt => dateTime()();
  DateTimeColumn get createdAt => dateTime()();
  TextColumn get status => text().withDefault(const Constant('PENDING'))();
  TextColumn get lastErrorCode => text().nullable()();
  @override
  Set<Column> get primaryKey => {id};
}

class SyncCursors extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get stream => text()();
  IntColumn get lastSequence => integer().withDefault(const Constant(0))();
  @override
  Set<Column> get primaryKey => {ownerUid, stream};
}

class CachedSmartCacheFiles extends Table {
  TextColumn get ownerUid => text()();
  TextColumn get roomId => text()();
  TextColumn get id => text()();
  TextColumn get sourceRelativePath => text()();
  TextColumn get payloadJson => text()();
  TextColumn get availabilityStatus => text()();
  TextColumn get freshnessStatus => text()();
  TextColumn get localDownloadPath => text().nullable()();
  TextColumn get sha256 => text().nullable()();
  IntColumn get sizeBytes => integer().nullable()();
  DateTimeColumn get lastVerifiedAt => dateTime().nullable()();
  DateTimeColumn get downloadedAt => dateTime().nullable()();
  DateTimeColumn get updatedAt => dateTime()();
  @override
  Set<Column> get primaryKey => {ownerUid, id};
}

@DriftDatabase(
  tables: [
    CachedDevices,
    CachedRooms,
    CachedCommands,
    CachedProposals,
    CachedExecutions,
    CachedRoomSnapshots,
    MutationOutbox,
    SyncCursors,
    CachedSmartCacheFiles,
  ],
)
class AppDatabase extends _$AppDatabase {
  AppDatabase() : super(_openConnection());
  AppDatabase.forTesting(super.executor);
  @override
  int get schemaVersion => 4;

  @override
  MigrationStrategy get migration => MigrationStrategy(
    onCreate: (migrator) => migrator.createAll(),
    onUpgrade: (migrator, from, to) async {
      if (from < 2) {
        await migrator.addColumn(mutationOutbox, mutationOutbox.status);
        await migrator.addColumn(mutationOutbox, mutationOutbox.lastErrorCode);
      }
      if (from < 3) {
        await migrator.deleteTable('cached_devices');
        await migrator.deleteTable('cached_rooms');
        await migrator.deleteTable('cached_commands');
        await migrator.deleteTable('cached_proposals');
        await migrator.deleteTable('mutation_outbox');
        await migrator.deleteTable('sync_cursors');
        await migrator.createTable(cachedDevices);
        await migrator.createTable(cachedRooms);
        await migrator.createTable(cachedCommands);
        await migrator.createTable(cachedProposals);
        await migrator.createTable(cachedExecutions);
        await migrator.createTable(cachedRoomSnapshots);
        await migrator.createTable(mutationOutbox);
        await migrator.createTable(syncCursors);
        await migrator.createTable(cachedSmartCacheFiles);
        return;
      }
      if (from < 4) {
        await migrator.createTable(cachedSmartCacheFiles);
      }
    },
  );
}

final appDatabaseProvider = Provider<AppDatabase>((ref) {
  final database = AppDatabase();
  ref.onDispose(database.close);
  return database;
});

LazyDatabase _openConnection() => LazyDatabase(() async {
  final directory = await getApplicationSupportDirectory();
  return NativeDatabase.createInBackground(
    File(p.join(directory.path, 'mousekeeper.sqlite')),
  );
});
