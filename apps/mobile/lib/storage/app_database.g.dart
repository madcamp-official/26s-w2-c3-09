// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'app_database.dart';

// ignore_for_file: type=lint
class $CachedDevicesTable extends CachedDevices
    with TableInfo<$CachedDevicesTable, CachedDevice> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $CachedDevicesTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<String> id = GeneratedColumn<String>(
    'id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _payloadJsonMeta = const VerificationMeta(
    'payloadJson',
  );
  @override
  late final GeneratedColumn<String> payloadJson = GeneratedColumn<String>(
    'payload_json',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [ownerUid, id, payloadJson, updatedAt];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'cached_devices';
  @override
  VerificationContext validateIntegrity(
    Insertable<CachedDevice> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    } else if (isInserting) {
      context.missing(_idMeta);
    }
    if (data.containsKey('payload_json')) {
      context.handle(
        _payloadJsonMeta,
        payloadJson.isAcceptableOrUnknown(
          data['payload_json']!,
          _payloadJsonMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_payloadJsonMeta);
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {ownerUid, id};
  @override
  CachedDevice map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return CachedDevice(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}id'],
      )!,
      payloadJson: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}payload_json'],
      )!,
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $CachedDevicesTable createAlias(String alias) {
    return $CachedDevicesTable(attachedDatabase, alias);
  }
}

class CachedDevice extends DataClass implements Insertable<CachedDevice> {
  final String ownerUid;
  final String id;
  final String payloadJson;
  final DateTime updatedAt;
  const CachedDevice({
    required this.ownerUid,
    required this.id,
    required this.payloadJson,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['id'] = Variable<String>(id);
    map['payload_json'] = Variable<String>(payloadJson);
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  CachedDevicesCompanion toCompanion(bool nullToAbsent) {
    return CachedDevicesCompanion(
      ownerUid: Value(ownerUid),
      id: Value(id),
      payloadJson: Value(payloadJson),
      updatedAt: Value(updatedAt),
    );
  }

  factory CachedDevice.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return CachedDevice(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      id: serializer.fromJson<String>(json['id']),
      payloadJson: serializer.fromJson<String>(json['payloadJson']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'id': serializer.toJson<String>(id),
      'payloadJson': serializer.toJson<String>(payloadJson),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  CachedDevice copyWith({
    String? ownerUid,
    String? id,
    String? payloadJson,
    DateTime? updatedAt,
  }) => CachedDevice(
    ownerUid: ownerUid ?? this.ownerUid,
    id: id ?? this.id,
    payloadJson: payloadJson ?? this.payloadJson,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  CachedDevice copyWithCompanion(CachedDevicesCompanion data) {
    return CachedDevice(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      id: data.id.present ? data.id.value : this.id,
      payloadJson: data.payloadJson.present
          ? data.payloadJson.value
          : this.payloadJson,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('CachedDevice(')
          ..write('ownerUid: $ownerUid, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(ownerUid, id, payloadJson, updatedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CachedDevice &&
          other.ownerUid == this.ownerUid &&
          other.id == this.id &&
          other.payloadJson == this.payloadJson &&
          other.updatedAt == this.updatedAt);
}

class CachedDevicesCompanion extends UpdateCompanion<CachedDevice> {
  final Value<String> ownerUid;
  final Value<String> id;
  final Value<String> payloadJson;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const CachedDevicesCompanion({
    this.ownerUid = const Value.absent(),
    this.id = const Value.absent(),
    this.payloadJson = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  CachedDevicesCompanion.insert({
    required String ownerUid,
    required String id,
    required String payloadJson,
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       id = Value(id),
       payloadJson = Value(payloadJson),
       updatedAt = Value(updatedAt);
  static Insertable<CachedDevice> custom({
    Expression<String>? ownerUid,
    Expression<String>? id,
    Expression<String>? payloadJson,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (id != null) 'id': id,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  CachedDevicesCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? id,
    Value<String>? payloadJson,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return CachedDevicesCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      id: id ?? this.id,
      payloadJson: payloadJson ?? this.payloadJson,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (id.present) {
      map['id'] = Variable<String>(id.value);
    }
    if (payloadJson.present) {
      map['payload_json'] = Variable<String>(payloadJson.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('CachedDevicesCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $CachedRoomsTable extends CachedRooms
    with TableInfo<$CachedRoomsTable, CachedRoom> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $CachedRoomsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<String> id = GeneratedColumn<String>(
    'id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _payloadJsonMeta = const VerificationMeta(
    'payloadJson',
  );
  @override
  late final GeneratedColumn<String> payloadJson = GeneratedColumn<String>(
    'payload_json',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [ownerUid, id, payloadJson, updatedAt];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'cached_rooms';
  @override
  VerificationContext validateIntegrity(
    Insertable<CachedRoom> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    } else if (isInserting) {
      context.missing(_idMeta);
    }
    if (data.containsKey('payload_json')) {
      context.handle(
        _payloadJsonMeta,
        payloadJson.isAcceptableOrUnknown(
          data['payload_json']!,
          _payloadJsonMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_payloadJsonMeta);
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {ownerUid, id};
  @override
  CachedRoom map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return CachedRoom(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}id'],
      )!,
      payloadJson: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}payload_json'],
      )!,
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $CachedRoomsTable createAlias(String alias) {
    return $CachedRoomsTable(attachedDatabase, alias);
  }
}

class CachedRoom extends DataClass implements Insertable<CachedRoom> {
  final String ownerUid;
  final String id;
  final String payloadJson;
  final DateTime updatedAt;
  const CachedRoom({
    required this.ownerUid,
    required this.id,
    required this.payloadJson,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['id'] = Variable<String>(id);
    map['payload_json'] = Variable<String>(payloadJson);
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  CachedRoomsCompanion toCompanion(bool nullToAbsent) {
    return CachedRoomsCompanion(
      ownerUid: Value(ownerUid),
      id: Value(id),
      payloadJson: Value(payloadJson),
      updatedAt: Value(updatedAt),
    );
  }

  factory CachedRoom.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return CachedRoom(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      id: serializer.fromJson<String>(json['id']),
      payloadJson: serializer.fromJson<String>(json['payloadJson']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'id': serializer.toJson<String>(id),
      'payloadJson': serializer.toJson<String>(payloadJson),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  CachedRoom copyWith({
    String? ownerUid,
    String? id,
    String? payloadJson,
    DateTime? updatedAt,
  }) => CachedRoom(
    ownerUid: ownerUid ?? this.ownerUid,
    id: id ?? this.id,
    payloadJson: payloadJson ?? this.payloadJson,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  CachedRoom copyWithCompanion(CachedRoomsCompanion data) {
    return CachedRoom(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      id: data.id.present ? data.id.value : this.id,
      payloadJson: data.payloadJson.present
          ? data.payloadJson.value
          : this.payloadJson,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('CachedRoom(')
          ..write('ownerUid: $ownerUid, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(ownerUid, id, payloadJson, updatedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CachedRoom &&
          other.ownerUid == this.ownerUid &&
          other.id == this.id &&
          other.payloadJson == this.payloadJson &&
          other.updatedAt == this.updatedAt);
}

class CachedRoomsCompanion extends UpdateCompanion<CachedRoom> {
  final Value<String> ownerUid;
  final Value<String> id;
  final Value<String> payloadJson;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const CachedRoomsCompanion({
    this.ownerUid = const Value.absent(),
    this.id = const Value.absent(),
    this.payloadJson = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  CachedRoomsCompanion.insert({
    required String ownerUid,
    required String id,
    required String payloadJson,
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       id = Value(id),
       payloadJson = Value(payloadJson),
       updatedAt = Value(updatedAt);
  static Insertable<CachedRoom> custom({
    Expression<String>? ownerUid,
    Expression<String>? id,
    Expression<String>? payloadJson,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (id != null) 'id': id,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  CachedRoomsCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? id,
    Value<String>? payloadJson,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return CachedRoomsCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      id: id ?? this.id,
      payloadJson: payloadJson ?? this.payloadJson,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (id.present) {
      map['id'] = Variable<String>(id.value);
    }
    if (payloadJson.present) {
      map['payload_json'] = Variable<String>(payloadJson.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('CachedRoomsCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $CachedCommandsTable extends CachedCommands
    with TableInfo<$CachedCommandsTable, CachedCommand> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $CachedCommandsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _roomIdMeta = const VerificationMeta('roomId');
  @override
  late final GeneratedColumn<String> roomId = GeneratedColumn<String>(
    'room_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<String> id = GeneratedColumn<String>(
    'id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _payloadJsonMeta = const VerificationMeta(
    'payloadJson',
  );
  @override
  late final GeneratedColumn<String> payloadJson = GeneratedColumn<String>(
    'payload_json',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [
    ownerUid,
    roomId,
    id,
    payloadJson,
    updatedAt,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'cached_commands';
  @override
  VerificationContext validateIntegrity(
    Insertable<CachedCommand> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('room_id')) {
      context.handle(
        _roomIdMeta,
        roomId.isAcceptableOrUnknown(data['room_id']!, _roomIdMeta),
      );
    } else if (isInserting) {
      context.missing(_roomIdMeta);
    }
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    } else if (isInserting) {
      context.missing(_idMeta);
    }
    if (data.containsKey('payload_json')) {
      context.handle(
        _payloadJsonMeta,
        payloadJson.isAcceptableOrUnknown(
          data['payload_json']!,
          _payloadJsonMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_payloadJsonMeta);
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {ownerUid, id};
  @override
  CachedCommand map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return CachedCommand(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      roomId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}room_id'],
      )!,
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}id'],
      )!,
      payloadJson: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}payload_json'],
      )!,
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $CachedCommandsTable createAlias(String alias) {
    return $CachedCommandsTable(attachedDatabase, alias);
  }
}

class CachedCommand extends DataClass implements Insertable<CachedCommand> {
  final String ownerUid;
  final String roomId;
  final String id;
  final String payloadJson;
  final DateTime updatedAt;
  const CachedCommand({
    required this.ownerUid,
    required this.roomId,
    required this.id,
    required this.payloadJson,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['room_id'] = Variable<String>(roomId);
    map['id'] = Variable<String>(id);
    map['payload_json'] = Variable<String>(payloadJson);
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  CachedCommandsCompanion toCompanion(bool nullToAbsent) {
    return CachedCommandsCompanion(
      ownerUid: Value(ownerUid),
      roomId: Value(roomId),
      id: Value(id),
      payloadJson: Value(payloadJson),
      updatedAt: Value(updatedAt),
    );
  }

  factory CachedCommand.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return CachedCommand(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      roomId: serializer.fromJson<String>(json['roomId']),
      id: serializer.fromJson<String>(json['id']),
      payloadJson: serializer.fromJson<String>(json['payloadJson']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'roomId': serializer.toJson<String>(roomId),
      'id': serializer.toJson<String>(id),
      'payloadJson': serializer.toJson<String>(payloadJson),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  CachedCommand copyWith({
    String? ownerUid,
    String? roomId,
    String? id,
    String? payloadJson,
    DateTime? updatedAt,
  }) => CachedCommand(
    ownerUid: ownerUid ?? this.ownerUid,
    roomId: roomId ?? this.roomId,
    id: id ?? this.id,
    payloadJson: payloadJson ?? this.payloadJson,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  CachedCommand copyWithCompanion(CachedCommandsCompanion data) {
    return CachedCommand(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      roomId: data.roomId.present ? data.roomId.value : this.roomId,
      id: data.id.present ? data.id.value : this.id,
      payloadJson: data.payloadJson.present
          ? data.payloadJson.value
          : this.payloadJson,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('CachedCommand(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(ownerUid, roomId, id, payloadJson, updatedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CachedCommand &&
          other.ownerUid == this.ownerUid &&
          other.roomId == this.roomId &&
          other.id == this.id &&
          other.payloadJson == this.payloadJson &&
          other.updatedAt == this.updatedAt);
}

class CachedCommandsCompanion extends UpdateCompanion<CachedCommand> {
  final Value<String> ownerUid;
  final Value<String> roomId;
  final Value<String> id;
  final Value<String> payloadJson;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const CachedCommandsCompanion({
    this.ownerUid = const Value.absent(),
    this.roomId = const Value.absent(),
    this.id = const Value.absent(),
    this.payloadJson = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  CachedCommandsCompanion.insert({
    required String ownerUid,
    required String roomId,
    required String id,
    required String payloadJson,
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       roomId = Value(roomId),
       id = Value(id),
       payloadJson = Value(payloadJson),
       updatedAt = Value(updatedAt);
  static Insertable<CachedCommand> custom({
    Expression<String>? ownerUid,
    Expression<String>? roomId,
    Expression<String>? id,
    Expression<String>? payloadJson,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (roomId != null) 'room_id': roomId,
      if (id != null) 'id': id,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  CachedCommandsCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? roomId,
    Value<String>? id,
    Value<String>? payloadJson,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return CachedCommandsCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      roomId: roomId ?? this.roomId,
      id: id ?? this.id,
      payloadJson: payloadJson ?? this.payloadJson,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (roomId.present) {
      map['room_id'] = Variable<String>(roomId.value);
    }
    if (id.present) {
      map['id'] = Variable<String>(id.value);
    }
    if (payloadJson.present) {
      map['payload_json'] = Variable<String>(payloadJson.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('CachedCommandsCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $CachedProposalsTable extends CachedProposals
    with TableInfo<$CachedProposalsTable, CachedProposal> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $CachedProposalsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _roomIdMeta = const VerificationMeta('roomId');
  @override
  late final GeneratedColumn<String> roomId = GeneratedColumn<String>(
    'room_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<String> id = GeneratedColumn<String>(
    'id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _payloadJsonMeta = const VerificationMeta(
    'payloadJson',
  );
  @override
  late final GeneratedColumn<String> payloadJson = GeneratedColumn<String>(
    'payload_json',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [
    ownerUid,
    roomId,
    id,
    payloadJson,
    updatedAt,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'cached_proposals';
  @override
  VerificationContext validateIntegrity(
    Insertable<CachedProposal> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('room_id')) {
      context.handle(
        _roomIdMeta,
        roomId.isAcceptableOrUnknown(data['room_id']!, _roomIdMeta),
      );
    } else if (isInserting) {
      context.missing(_roomIdMeta);
    }
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    } else if (isInserting) {
      context.missing(_idMeta);
    }
    if (data.containsKey('payload_json')) {
      context.handle(
        _payloadJsonMeta,
        payloadJson.isAcceptableOrUnknown(
          data['payload_json']!,
          _payloadJsonMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_payloadJsonMeta);
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {ownerUid, id};
  @override
  CachedProposal map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return CachedProposal(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      roomId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}room_id'],
      )!,
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}id'],
      )!,
      payloadJson: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}payload_json'],
      )!,
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $CachedProposalsTable createAlias(String alias) {
    return $CachedProposalsTable(attachedDatabase, alias);
  }
}

class CachedProposal extends DataClass implements Insertable<CachedProposal> {
  final String ownerUid;
  final String roomId;
  final String id;
  final String payloadJson;
  final DateTime updatedAt;
  const CachedProposal({
    required this.ownerUid,
    required this.roomId,
    required this.id,
    required this.payloadJson,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['room_id'] = Variable<String>(roomId);
    map['id'] = Variable<String>(id);
    map['payload_json'] = Variable<String>(payloadJson);
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  CachedProposalsCompanion toCompanion(bool nullToAbsent) {
    return CachedProposalsCompanion(
      ownerUid: Value(ownerUid),
      roomId: Value(roomId),
      id: Value(id),
      payloadJson: Value(payloadJson),
      updatedAt: Value(updatedAt),
    );
  }

  factory CachedProposal.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return CachedProposal(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      roomId: serializer.fromJson<String>(json['roomId']),
      id: serializer.fromJson<String>(json['id']),
      payloadJson: serializer.fromJson<String>(json['payloadJson']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'roomId': serializer.toJson<String>(roomId),
      'id': serializer.toJson<String>(id),
      'payloadJson': serializer.toJson<String>(payloadJson),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  CachedProposal copyWith({
    String? ownerUid,
    String? roomId,
    String? id,
    String? payloadJson,
    DateTime? updatedAt,
  }) => CachedProposal(
    ownerUid: ownerUid ?? this.ownerUid,
    roomId: roomId ?? this.roomId,
    id: id ?? this.id,
    payloadJson: payloadJson ?? this.payloadJson,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  CachedProposal copyWithCompanion(CachedProposalsCompanion data) {
    return CachedProposal(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      roomId: data.roomId.present ? data.roomId.value : this.roomId,
      id: data.id.present ? data.id.value : this.id,
      payloadJson: data.payloadJson.present
          ? data.payloadJson.value
          : this.payloadJson,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('CachedProposal(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(ownerUid, roomId, id, payloadJson, updatedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CachedProposal &&
          other.ownerUid == this.ownerUid &&
          other.roomId == this.roomId &&
          other.id == this.id &&
          other.payloadJson == this.payloadJson &&
          other.updatedAt == this.updatedAt);
}

class CachedProposalsCompanion extends UpdateCompanion<CachedProposal> {
  final Value<String> ownerUid;
  final Value<String> roomId;
  final Value<String> id;
  final Value<String> payloadJson;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const CachedProposalsCompanion({
    this.ownerUid = const Value.absent(),
    this.roomId = const Value.absent(),
    this.id = const Value.absent(),
    this.payloadJson = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  CachedProposalsCompanion.insert({
    required String ownerUid,
    required String roomId,
    required String id,
    required String payloadJson,
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       roomId = Value(roomId),
       id = Value(id),
       payloadJson = Value(payloadJson),
       updatedAt = Value(updatedAt);
  static Insertable<CachedProposal> custom({
    Expression<String>? ownerUid,
    Expression<String>? roomId,
    Expression<String>? id,
    Expression<String>? payloadJson,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (roomId != null) 'room_id': roomId,
      if (id != null) 'id': id,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  CachedProposalsCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? roomId,
    Value<String>? id,
    Value<String>? payloadJson,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return CachedProposalsCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      roomId: roomId ?? this.roomId,
      id: id ?? this.id,
      payloadJson: payloadJson ?? this.payloadJson,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (roomId.present) {
      map['room_id'] = Variable<String>(roomId.value);
    }
    if (id.present) {
      map['id'] = Variable<String>(id.value);
    }
    if (payloadJson.present) {
      map['payload_json'] = Variable<String>(payloadJson.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('CachedProposalsCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $CachedExecutionsTable extends CachedExecutions
    with TableInfo<$CachedExecutionsTable, CachedExecution> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $CachedExecutionsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _roomIdMeta = const VerificationMeta('roomId');
  @override
  late final GeneratedColumn<String> roomId = GeneratedColumn<String>(
    'room_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<String> id = GeneratedColumn<String>(
    'id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _payloadJsonMeta = const VerificationMeta(
    'payloadJson',
  );
  @override
  late final GeneratedColumn<String> payloadJson = GeneratedColumn<String>(
    'payload_json',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [
    ownerUid,
    roomId,
    id,
    payloadJson,
    updatedAt,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'cached_executions';
  @override
  VerificationContext validateIntegrity(
    Insertable<CachedExecution> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('room_id')) {
      context.handle(
        _roomIdMeta,
        roomId.isAcceptableOrUnknown(data['room_id']!, _roomIdMeta),
      );
    } else if (isInserting) {
      context.missing(_roomIdMeta);
    }
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    } else if (isInserting) {
      context.missing(_idMeta);
    }
    if (data.containsKey('payload_json')) {
      context.handle(
        _payloadJsonMeta,
        payloadJson.isAcceptableOrUnknown(
          data['payload_json']!,
          _payloadJsonMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_payloadJsonMeta);
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {ownerUid, id};
  @override
  CachedExecution map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return CachedExecution(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      roomId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}room_id'],
      )!,
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}id'],
      )!,
      payloadJson: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}payload_json'],
      )!,
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $CachedExecutionsTable createAlias(String alias) {
    return $CachedExecutionsTable(attachedDatabase, alias);
  }
}

class CachedExecution extends DataClass implements Insertable<CachedExecution> {
  final String ownerUid;
  final String roomId;
  final String id;
  final String payloadJson;
  final DateTime updatedAt;
  const CachedExecution({
    required this.ownerUid,
    required this.roomId,
    required this.id,
    required this.payloadJson,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['room_id'] = Variable<String>(roomId);
    map['id'] = Variable<String>(id);
    map['payload_json'] = Variable<String>(payloadJson);
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  CachedExecutionsCompanion toCompanion(bool nullToAbsent) {
    return CachedExecutionsCompanion(
      ownerUid: Value(ownerUid),
      roomId: Value(roomId),
      id: Value(id),
      payloadJson: Value(payloadJson),
      updatedAt: Value(updatedAt),
    );
  }

  factory CachedExecution.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return CachedExecution(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      roomId: serializer.fromJson<String>(json['roomId']),
      id: serializer.fromJson<String>(json['id']),
      payloadJson: serializer.fromJson<String>(json['payloadJson']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'roomId': serializer.toJson<String>(roomId),
      'id': serializer.toJson<String>(id),
      'payloadJson': serializer.toJson<String>(payloadJson),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  CachedExecution copyWith({
    String? ownerUid,
    String? roomId,
    String? id,
    String? payloadJson,
    DateTime? updatedAt,
  }) => CachedExecution(
    ownerUid: ownerUid ?? this.ownerUid,
    roomId: roomId ?? this.roomId,
    id: id ?? this.id,
    payloadJson: payloadJson ?? this.payloadJson,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  CachedExecution copyWithCompanion(CachedExecutionsCompanion data) {
    return CachedExecution(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      roomId: data.roomId.present ? data.roomId.value : this.roomId,
      id: data.id.present ? data.id.value : this.id,
      payloadJson: data.payloadJson.present
          ? data.payloadJson.value
          : this.payloadJson,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('CachedExecution(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(ownerUid, roomId, id, payloadJson, updatedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CachedExecution &&
          other.ownerUid == this.ownerUid &&
          other.roomId == this.roomId &&
          other.id == this.id &&
          other.payloadJson == this.payloadJson &&
          other.updatedAt == this.updatedAt);
}

class CachedExecutionsCompanion extends UpdateCompanion<CachedExecution> {
  final Value<String> ownerUid;
  final Value<String> roomId;
  final Value<String> id;
  final Value<String> payloadJson;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const CachedExecutionsCompanion({
    this.ownerUid = const Value.absent(),
    this.roomId = const Value.absent(),
    this.id = const Value.absent(),
    this.payloadJson = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  CachedExecutionsCompanion.insert({
    required String ownerUid,
    required String roomId,
    required String id,
    required String payloadJson,
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       roomId = Value(roomId),
       id = Value(id),
       payloadJson = Value(payloadJson),
       updatedAt = Value(updatedAt);
  static Insertable<CachedExecution> custom({
    Expression<String>? ownerUid,
    Expression<String>? roomId,
    Expression<String>? id,
    Expression<String>? payloadJson,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (roomId != null) 'room_id': roomId,
      if (id != null) 'id': id,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  CachedExecutionsCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? roomId,
    Value<String>? id,
    Value<String>? payloadJson,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return CachedExecutionsCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      roomId: roomId ?? this.roomId,
      id: id ?? this.id,
      payloadJson: payloadJson ?? this.payloadJson,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (roomId.present) {
      map['room_id'] = Variable<String>(roomId.value);
    }
    if (id.present) {
      map['id'] = Variable<String>(id.value);
    }
    if (payloadJson.present) {
      map['payload_json'] = Variable<String>(payloadJson.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('CachedExecutionsCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('id: $id, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $CachedRoomSnapshotsTable extends CachedRoomSnapshots
    with TableInfo<$CachedRoomSnapshotsTable, CachedRoomSnapshot> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $CachedRoomSnapshotsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _roomIdMeta = const VerificationMeta('roomId');
  @override
  late final GeneratedColumn<String> roomId = GeneratedColumn<String>(
    'room_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _payloadJsonMeta = const VerificationMeta(
    'payloadJson',
  );
  @override
  late final GeneratedColumn<String> payloadJson = GeneratedColumn<String>(
    'payload_json',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [
    ownerUid,
    roomId,
    payloadJson,
    updatedAt,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'cached_room_snapshots';
  @override
  VerificationContext validateIntegrity(
    Insertable<CachedRoomSnapshot> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('room_id')) {
      context.handle(
        _roomIdMeta,
        roomId.isAcceptableOrUnknown(data['room_id']!, _roomIdMeta),
      );
    } else if (isInserting) {
      context.missing(_roomIdMeta);
    }
    if (data.containsKey('payload_json')) {
      context.handle(
        _payloadJsonMeta,
        payloadJson.isAcceptableOrUnknown(
          data['payload_json']!,
          _payloadJsonMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_payloadJsonMeta);
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {ownerUid, roomId};
  @override
  CachedRoomSnapshot map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return CachedRoomSnapshot(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      roomId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}room_id'],
      )!,
      payloadJson: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}payload_json'],
      )!,
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $CachedRoomSnapshotsTable createAlias(String alias) {
    return $CachedRoomSnapshotsTable(attachedDatabase, alias);
  }
}

class CachedRoomSnapshot extends DataClass
    implements Insertable<CachedRoomSnapshot> {
  final String ownerUid;
  final String roomId;
  final String payloadJson;
  final DateTime updatedAt;
  const CachedRoomSnapshot({
    required this.ownerUid,
    required this.roomId,
    required this.payloadJson,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['room_id'] = Variable<String>(roomId);
    map['payload_json'] = Variable<String>(payloadJson);
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  CachedRoomSnapshotsCompanion toCompanion(bool nullToAbsent) {
    return CachedRoomSnapshotsCompanion(
      ownerUid: Value(ownerUid),
      roomId: Value(roomId),
      payloadJson: Value(payloadJson),
      updatedAt: Value(updatedAt),
    );
  }

  factory CachedRoomSnapshot.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return CachedRoomSnapshot(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      roomId: serializer.fromJson<String>(json['roomId']),
      payloadJson: serializer.fromJson<String>(json['payloadJson']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'roomId': serializer.toJson<String>(roomId),
      'payloadJson': serializer.toJson<String>(payloadJson),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  CachedRoomSnapshot copyWith({
    String? ownerUid,
    String? roomId,
    String? payloadJson,
    DateTime? updatedAt,
  }) => CachedRoomSnapshot(
    ownerUid: ownerUid ?? this.ownerUid,
    roomId: roomId ?? this.roomId,
    payloadJson: payloadJson ?? this.payloadJson,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  CachedRoomSnapshot copyWithCompanion(CachedRoomSnapshotsCompanion data) {
    return CachedRoomSnapshot(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      roomId: data.roomId.present ? data.roomId.value : this.roomId,
      payloadJson: data.payloadJson.present
          ? data.payloadJson.value
          : this.payloadJson,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('CachedRoomSnapshot(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(ownerUid, roomId, payloadJson, updatedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CachedRoomSnapshot &&
          other.ownerUid == this.ownerUid &&
          other.roomId == this.roomId &&
          other.payloadJson == this.payloadJson &&
          other.updatedAt == this.updatedAt);
}

class CachedRoomSnapshotsCompanion extends UpdateCompanion<CachedRoomSnapshot> {
  final Value<String> ownerUid;
  final Value<String> roomId;
  final Value<String> payloadJson;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const CachedRoomSnapshotsCompanion({
    this.ownerUid = const Value.absent(),
    this.roomId = const Value.absent(),
    this.payloadJson = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  CachedRoomSnapshotsCompanion.insert({
    required String ownerUid,
    required String roomId,
    required String payloadJson,
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       roomId = Value(roomId),
       payloadJson = Value(payloadJson),
       updatedAt = Value(updatedAt);
  static Insertable<CachedRoomSnapshot> custom({
    Expression<String>? ownerUid,
    Expression<String>? roomId,
    Expression<String>? payloadJson,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (roomId != null) 'room_id': roomId,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  CachedRoomSnapshotsCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? roomId,
    Value<String>? payloadJson,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return CachedRoomSnapshotsCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      roomId: roomId ?? this.roomId,
      payloadJson: payloadJson ?? this.payloadJson,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (roomId.present) {
      map['room_id'] = Variable<String>(roomId.value);
    }
    if (payloadJson.present) {
      map['payload_json'] = Variable<String>(payloadJson.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('CachedRoomSnapshotsCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $MutationOutboxTable extends MutationOutbox
    with TableInfo<$MutationOutboxTable, MutationOutboxData> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $MutationOutboxTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<String> id = GeneratedColumn<String>(
    'id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _mutationTypeMeta = const VerificationMeta(
    'mutationType',
  );
  @override
  late final GeneratedColumn<String> mutationType = GeneratedColumn<String>(
    'mutation_type',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _payloadJsonMeta = const VerificationMeta(
    'payloadJson',
  );
  @override
  late final GeneratedColumn<String> payloadJson = GeneratedColumn<String>(
    'payload_json',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _attemptCountMeta = const VerificationMeta(
    'attemptCount',
  );
  @override
  late final GeneratedColumn<int> attemptCount = GeneratedColumn<int>(
    'attempt_count',
    aliasedName,
    false,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
    defaultValue: const Constant(0),
  );
  static const VerificationMeta _nextRetryAtMeta = const VerificationMeta(
    'nextRetryAt',
  );
  @override
  late final GeneratedColumn<DateTime> nextRetryAt = GeneratedColumn<DateTime>(
    'next_retry_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _createdAtMeta = const VerificationMeta(
    'createdAt',
  );
  @override
  late final GeneratedColumn<DateTime> createdAt = GeneratedColumn<DateTime>(
    'created_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _statusMeta = const VerificationMeta('status');
  @override
  late final GeneratedColumn<String> status = GeneratedColumn<String>(
    'status',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
    defaultValue: const Constant('PENDING'),
  );
  static const VerificationMeta _lastErrorCodeMeta = const VerificationMeta(
    'lastErrorCode',
  );
  @override
  late final GeneratedColumn<String> lastErrorCode = GeneratedColumn<String>(
    'last_error_code',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  @override
  List<GeneratedColumn> get $columns => [
    ownerUid,
    id,
    mutationType,
    payloadJson,
    attemptCount,
    nextRetryAt,
    createdAt,
    status,
    lastErrorCode,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'mutation_outbox';
  @override
  VerificationContext validateIntegrity(
    Insertable<MutationOutboxData> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    } else if (isInserting) {
      context.missing(_idMeta);
    }
    if (data.containsKey('mutation_type')) {
      context.handle(
        _mutationTypeMeta,
        mutationType.isAcceptableOrUnknown(
          data['mutation_type']!,
          _mutationTypeMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_mutationTypeMeta);
    }
    if (data.containsKey('payload_json')) {
      context.handle(
        _payloadJsonMeta,
        payloadJson.isAcceptableOrUnknown(
          data['payload_json']!,
          _payloadJsonMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_payloadJsonMeta);
    }
    if (data.containsKey('attempt_count')) {
      context.handle(
        _attemptCountMeta,
        attemptCount.isAcceptableOrUnknown(
          data['attempt_count']!,
          _attemptCountMeta,
        ),
      );
    }
    if (data.containsKey('next_retry_at')) {
      context.handle(
        _nextRetryAtMeta,
        nextRetryAt.isAcceptableOrUnknown(
          data['next_retry_at']!,
          _nextRetryAtMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_nextRetryAtMeta);
    }
    if (data.containsKey('created_at')) {
      context.handle(
        _createdAtMeta,
        createdAt.isAcceptableOrUnknown(data['created_at']!, _createdAtMeta),
      );
    } else if (isInserting) {
      context.missing(_createdAtMeta);
    }
    if (data.containsKey('status')) {
      context.handle(
        _statusMeta,
        status.isAcceptableOrUnknown(data['status']!, _statusMeta),
      );
    }
    if (data.containsKey('last_error_code')) {
      context.handle(
        _lastErrorCodeMeta,
        lastErrorCode.isAcceptableOrUnknown(
          data['last_error_code']!,
          _lastErrorCodeMeta,
        ),
      );
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {id};
  @override
  MutationOutboxData map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return MutationOutboxData(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}id'],
      )!,
      mutationType: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}mutation_type'],
      )!,
      payloadJson: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}payload_json'],
      )!,
      attemptCount: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}attempt_count'],
      )!,
      nextRetryAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}next_retry_at'],
      )!,
      createdAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}created_at'],
      )!,
      status: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}status'],
      )!,
      lastErrorCode: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}last_error_code'],
      ),
    );
  }

  @override
  $MutationOutboxTable createAlias(String alias) {
    return $MutationOutboxTable(attachedDatabase, alias);
  }
}

class MutationOutboxData extends DataClass
    implements Insertable<MutationOutboxData> {
  final String ownerUid;
  final String id;
  final String mutationType;
  final String payloadJson;
  final int attemptCount;
  final DateTime nextRetryAt;
  final DateTime createdAt;
  final String status;
  final String? lastErrorCode;
  const MutationOutboxData({
    required this.ownerUid,
    required this.id,
    required this.mutationType,
    required this.payloadJson,
    required this.attemptCount,
    required this.nextRetryAt,
    required this.createdAt,
    required this.status,
    this.lastErrorCode,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['id'] = Variable<String>(id);
    map['mutation_type'] = Variable<String>(mutationType);
    map['payload_json'] = Variable<String>(payloadJson);
    map['attempt_count'] = Variable<int>(attemptCount);
    map['next_retry_at'] = Variable<DateTime>(nextRetryAt);
    map['created_at'] = Variable<DateTime>(createdAt);
    map['status'] = Variable<String>(status);
    if (!nullToAbsent || lastErrorCode != null) {
      map['last_error_code'] = Variable<String>(lastErrorCode);
    }
    return map;
  }

  MutationOutboxCompanion toCompanion(bool nullToAbsent) {
    return MutationOutboxCompanion(
      ownerUid: Value(ownerUid),
      id: Value(id),
      mutationType: Value(mutationType),
      payloadJson: Value(payloadJson),
      attemptCount: Value(attemptCount),
      nextRetryAt: Value(nextRetryAt),
      createdAt: Value(createdAt),
      status: Value(status),
      lastErrorCode: lastErrorCode == null && nullToAbsent
          ? const Value.absent()
          : Value(lastErrorCode),
    );
  }

  factory MutationOutboxData.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return MutationOutboxData(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      id: serializer.fromJson<String>(json['id']),
      mutationType: serializer.fromJson<String>(json['mutationType']),
      payloadJson: serializer.fromJson<String>(json['payloadJson']),
      attemptCount: serializer.fromJson<int>(json['attemptCount']),
      nextRetryAt: serializer.fromJson<DateTime>(json['nextRetryAt']),
      createdAt: serializer.fromJson<DateTime>(json['createdAt']),
      status: serializer.fromJson<String>(json['status']),
      lastErrorCode: serializer.fromJson<String?>(json['lastErrorCode']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'id': serializer.toJson<String>(id),
      'mutationType': serializer.toJson<String>(mutationType),
      'payloadJson': serializer.toJson<String>(payloadJson),
      'attemptCount': serializer.toJson<int>(attemptCount),
      'nextRetryAt': serializer.toJson<DateTime>(nextRetryAt),
      'createdAt': serializer.toJson<DateTime>(createdAt),
      'status': serializer.toJson<String>(status),
      'lastErrorCode': serializer.toJson<String?>(lastErrorCode),
    };
  }

  MutationOutboxData copyWith({
    String? ownerUid,
    String? id,
    String? mutationType,
    String? payloadJson,
    int? attemptCount,
    DateTime? nextRetryAt,
    DateTime? createdAt,
    String? status,
    Value<String?> lastErrorCode = const Value.absent(),
  }) => MutationOutboxData(
    ownerUid: ownerUid ?? this.ownerUid,
    id: id ?? this.id,
    mutationType: mutationType ?? this.mutationType,
    payloadJson: payloadJson ?? this.payloadJson,
    attemptCount: attemptCount ?? this.attemptCount,
    nextRetryAt: nextRetryAt ?? this.nextRetryAt,
    createdAt: createdAt ?? this.createdAt,
    status: status ?? this.status,
    lastErrorCode: lastErrorCode.present
        ? lastErrorCode.value
        : this.lastErrorCode,
  );
  MutationOutboxData copyWithCompanion(MutationOutboxCompanion data) {
    return MutationOutboxData(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      id: data.id.present ? data.id.value : this.id,
      mutationType: data.mutationType.present
          ? data.mutationType.value
          : this.mutationType,
      payloadJson: data.payloadJson.present
          ? data.payloadJson.value
          : this.payloadJson,
      attemptCount: data.attemptCount.present
          ? data.attemptCount.value
          : this.attemptCount,
      nextRetryAt: data.nextRetryAt.present
          ? data.nextRetryAt.value
          : this.nextRetryAt,
      createdAt: data.createdAt.present ? data.createdAt.value : this.createdAt,
      status: data.status.present ? data.status.value : this.status,
      lastErrorCode: data.lastErrorCode.present
          ? data.lastErrorCode.value
          : this.lastErrorCode,
    );
  }

  @override
  String toString() {
    return (StringBuffer('MutationOutboxData(')
          ..write('ownerUid: $ownerUid, ')
          ..write('id: $id, ')
          ..write('mutationType: $mutationType, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('attemptCount: $attemptCount, ')
          ..write('nextRetryAt: $nextRetryAt, ')
          ..write('createdAt: $createdAt, ')
          ..write('status: $status, ')
          ..write('lastErrorCode: $lastErrorCode')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
    ownerUid,
    id,
    mutationType,
    payloadJson,
    attemptCount,
    nextRetryAt,
    createdAt,
    status,
    lastErrorCode,
  );
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is MutationOutboxData &&
          other.ownerUid == this.ownerUid &&
          other.id == this.id &&
          other.mutationType == this.mutationType &&
          other.payloadJson == this.payloadJson &&
          other.attemptCount == this.attemptCount &&
          other.nextRetryAt == this.nextRetryAt &&
          other.createdAt == this.createdAt &&
          other.status == this.status &&
          other.lastErrorCode == this.lastErrorCode);
}

class MutationOutboxCompanion extends UpdateCompanion<MutationOutboxData> {
  final Value<String> ownerUid;
  final Value<String> id;
  final Value<String> mutationType;
  final Value<String> payloadJson;
  final Value<int> attemptCount;
  final Value<DateTime> nextRetryAt;
  final Value<DateTime> createdAt;
  final Value<String> status;
  final Value<String?> lastErrorCode;
  final Value<int> rowid;
  const MutationOutboxCompanion({
    this.ownerUid = const Value.absent(),
    this.id = const Value.absent(),
    this.mutationType = const Value.absent(),
    this.payloadJson = const Value.absent(),
    this.attemptCount = const Value.absent(),
    this.nextRetryAt = const Value.absent(),
    this.createdAt = const Value.absent(),
    this.status = const Value.absent(),
    this.lastErrorCode = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  MutationOutboxCompanion.insert({
    required String ownerUid,
    required String id,
    required String mutationType,
    required String payloadJson,
    this.attemptCount = const Value.absent(),
    required DateTime nextRetryAt,
    required DateTime createdAt,
    this.status = const Value.absent(),
    this.lastErrorCode = const Value.absent(),
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       id = Value(id),
       mutationType = Value(mutationType),
       payloadJson = Value(payloadJson),
       nextRetryAt = Value(nextRetryAt),
       createdAt = Value(createdAt);
  static Insertable<MutationOutboxData> custom({
    Expression<String>? ownerUid,
    Expression<String>? id,
    Expression<String>? mutationType,
    Expression<String>? payloadJson,
    Expression<int>? attemptCount,
    Expression<DateTime>? nextRetryAt,
    Expression<DateTime>? createdAt,
    Expression<String>? status,
    Expression<String>? lastErrorCode,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (id != null) 'id': id,
      if (mutationType != null) 'mutation_type': mutationType,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (attemptCount != null) 'attempt_count': attemptCount,
      if (nextRetryAt != null) 'next_retry_at': nextRetryAt,
      if (createdAt != null) 'created_at': createdAt,
      if (status != null) 'status': status,
      if (lastErrorCode != null) 'last_error_code': lastErrorCode,
      if (rowid != null) 'rowid': rowid,
    });
  }

  MutationOutboxCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? id,
    Value<String>? mutationType,
    Value<String>? payloadJson,
    Value<int>? attemptCount,
    Value<DateTime>? nextRetryAt,
    Value<DateTime>? createdAt,
    Value<String>? status,
    Value<String?>? lastErrorCode,
    Value<int>? rowid,
  }) {
    return MutationOutboxCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      id: id ?? this.id,
      mutationType: mutationType ?? this.mutationType,
      payloadJson: payloadJson ?? this.payloadJson,
      attemptCount: attemptCount ?? this.attemptCount,
      nextRetryAt: nextRetryAt ?? this.nextRetryAt,
      createdAt: createdAt ?? this.createdAt,
      status: status ?? this.status,
      lastErrorCode: lastErrorCode ?? this.lastErrorCode,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (id.present) {
      map['id'] = Variable<String>(id.value);
    }
    if (mutationType.present) {
      map['mutation_type'] = Variable<String>(mutationType.value);
    }
    if (payloadJson.present) {
      map['payload_json'] = Variable<String>(payloadJson.value);
    }
    if (attemptCount.present) {
      map['attempt_count'] = Variable<int>(attemptCount.value);
    }
    if (nextRetryAt.present) {
      map['next_retry_at'] = Variable<DateTime>(nextRetryAt.value);
    }
    if (createdAt.present) {
      map['created_at'] = Variable<DateTime>(createdAt.value);
    }
    if (status.present) {
      map['status'] = Variable<String>(status.value);
    }
    if (lastErrorCode.present) {
      map['last_error_code'] = Variable<String>(lastErrorCode.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('MutationOutboxCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('id: $id, ')
          ..write('mutationType: $mutationType, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('attemptCount: $attemptCount, ')
          ..write('nextRetryAt: $nextRetryAt, ')
          ..write('createdAt: $createdAt, ')
          ..write('status: $status, ')
          ..write('lastErrorCode: $lastErrorCode, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $SyncCursorsTable extends SyncCursors
    with TableInfo<$SyncCursorsTable, SyncCursor> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $SyncCursorsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _streamMeta = const VerificationMeta('stream');
  @override
  late final GeneratedColumn<String> stream = GeneratedColumn<String>(
    'stream',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _lastSequenceMeta = const VerificationMeta(
    'lastSequence',
  );
  @override
  late final GeneratedColumn<int> lastSequence = GeneratedColumn<int>(
    'last_sequence',
    aliasedName,
    false,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
    defaultValue: const Constant(0),
  );
  @override
  List<GeneratedColumn> get $columns => [ownerUid, stream, lastSequence];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'sync_cursors';
  @override
  VerificationContext validateIntegrity(
    Insertable<SyncCursor> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('stream')) {
      context.handle(
        _streamMeta,
        stream.isAcceptableOrUnknown(data['stream']!, _streamMeta),
      );
    } else if (isInserting) {
      context.missing(_streamMeta);
    }
    if (data.containsKey('last_sequence')) {
      context.handle(
        _lastSequenceMeta,
        lastSequence.isAcceptableOrUnknown(
          data['last_sequence']!,
          _lastSequenceMeta,
        ),
      );
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {ownerUid, stream};
  @override
  SyncCursor map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return SyncCursor(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      stream: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}stream'],
      )!,
      lastSequence: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}last_sequence'],
      )!,
    );
  }

  @override
  $SyncCursorsTable createAlias(String alias) {
    return $SyncCursorsTable(attachedDatabase, alias);
  }
}

class SyncCursor extends DataClass implements Insertable<SyncCursor> {
  final String ownerUid;
  final String stream;
  final int lastSequence;
  const SyncCursor({
    required this.ownerUid,
    required this.stream,
    required this.lastSequence,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['stream'] = Variable<String>(stream);
    map['last_sequence'] = Variable<int>(lastSequence);
    return map;
  }

  SyncCursorsCompanion toCompanion(bool nullToAbsent) {
    return SyncCursorsCompanion(
      ownerUid: Value(ownerUid),
      stream: Value(stream),
      lastSequence: Value(lastSequence),
    );
  }

  factory SyncCursor.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return SyncCursor(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      stream: serializer.fromJson<String>(json['stream']),
      lastSequence: serializer.fromJson<int>(json['lastSequence']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'stream': serializer.toJson<String>(stream),
      'lastSequence': serializer.toJson<int>(lastSequence),
    };
  }

  SyncCursor copyWith({String? ownerUid, String? stream, int? lastSequence}) =>
      SyncCursor(
        ownerUid: ownerUid ?? this.ownerUid,
        stream: stream ?? this.stream,
        lastSequence: lastSequence ?? this.lastSequence,
      );
  SyncCursor copyWithCompanion(SyncCursorsCompanion data) {
    return SyncCursor(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      stream: data.stream.present ? data.stream.value : this.stream,
      lastSequence: data.lastSequence.present
          ? data.lastSequence.value
          : this.lastSequence,
    );
  }

  @override
  String toString() {
    return (StringBuffer('SyncCursor(')
          ..write('ownerUid: $ownerUid, ')
          ..write('stream: $stream, ')
          ..write('lastSequence: $lastSequence')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(ownerUid, stream, lastSequence);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is SyncCursor &&
          other.ownerUid == this.ownerUid &&
          other.stream == this.stream &&
          other.lastSequence == this.lastSequence);
}

class SyncCursorsCompanion extends UpdateCompanion<SyncCursor> {
  final Value<String> ownerUid;
  final Value<String> stream;
  final Value<int> lastSequence;
  final Value<int> rowid;
  const SyncCursorsCompanion({
    this.ownerUid = const Value.absent(),
    this.stream = const Value.absent(),
    this.lastSequence = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  SyncCursorsCompanion.insert({
    required String ownerUid,
    required String stream,
    this.lastSequence = const Value.absent(),
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       stream = Value(stream);
  static Insertable<SyncCursor> custom({
    Expression<String>? ownerUid,
    Expression<String>? stream,
    Expression<int>? lastSequence,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (stream != null) 'stream': stream,
      if (lastSequence != null) 'last_sequence': lastSequence,
      if (rowid != null) 'rowid': rowid,
    });
  }

  SyncCursorsCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? stream,
    Value<int>? lastSequence,
    Value<int>? rowid,
  }) {
    return SyncCursorsCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      stream: stream ?? this.stream,
      lastSequence: lastSequence ?? this.lastSequence,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (stream.present) {
      map['stream'] = Variable<String>(stream.value);
    }
    if (lastSequence.present) {
      map['last_sequence'] = Variable<int>(lastSequence.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('SyncCursorsCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('stream: $stream, ')
          ..write('lastSequence: $lastSequence, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $CachedSmartCacheFilesTable extends CachedSmartCacheFiles
    with TableInfo<$CachedSmartCacheFilesTable, CachedSmartCacheFile> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $CachedSmartCacheFilesTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _ownerUidMeta = const VerificationMeta(
    'ownerUid',
  );
  @override
  late final GeneratedColumn<String> ownerUid = GeneratedColumn<String>(
    'owner_uid',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _roomIdMeta = const VerificationMeta('roomId');
  @override
  late final GeneratedColumn<String> roomId = GeneratedColumn<String>(
    'room_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<String> id = GeneratedColumn<String>(
    'id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _sourceRelativePathMeta =
      const VerificationMeta('sourceRelativePath');
  @override
  late final GeneratedColumn<String> sourceRelativePath =
      GeneratedColumn<String>(
        'source_relative_path',
        aliasedName,
        false,
        type: DriftSqlType.string,
        requiredDuringInsert: true,
      );
  static const VerificationMeta _payloadJsonMeta = const VerificationMeta(
    'payloadJson',
  );
  @override
  late final GeneratedColumn<String> payloadJson = GeneratedColumn<String>(
    'payload_json',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _availabilityStatusMeta =
      const VerificationMeta('availabilityStatus');
  @override
  late final GeneratedColumn<String> availabilityStatus =
      GeneratedColumn<String>(
        'availability_status',
        aliasedName,
        false,
        type: DriftSqlType.string,
        requiredDuringInsert: true,
      );
  static const VerificationMeta _freshnessStatusMeta = const VerificationMeta(
    'freshnessStatus',
  );
  @override
  late final GeneratedColumn<String> freshnessStatus = GeneratedColumn<String>(
    'freshness_status',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _localDownloadPathMeta = const VerificationMeta(
    'localDownloadPath',
  );
  @override
  late final GeneratedColumn<String> localDownloadPath =
      GeneratedColumn<String>(
        'local_download_path',
        aliasedName,
        true,
        type: DriftSqlType.string,
        requiredDuringInsert: false,
      );
  static const VerificationMeta _sha256Meta = const VerificationMeta('sha256');
  @override
  late final GeneratedColumn<String> sha256 = GeneratedColumn<String>(
    'sha256',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _sizeBytesMeta = const VerificationMeta(
    'sizeBytes',
  );
  @override
  late final GeneratedColumn<int> sizeBytes = GeneratedColumn<int>(
    'size_bytes',
    aliasedName,
    true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _lastVerifiedAtMeta = const VerificationMeta(
    'lastVerifiedAt',
  );
  @override
  late final GeneratedColumn<DateTime> lastVerifiedAt =
      GeneratedColumn<DateTime>(
        'last_verified_at',
        aliasedName,
        true,
        type: DriftSqlType.dateTime,
        requiredDuringInsert: false,
      );
  static const VerificationMeta _downloadedAtMeta = const VerificationMeta(
    'downloadedAt',
  );
  @override
  late final GeneratedColumn<DateTime> downloadedAt = GeneratedColumn<DateTime>(
    'downloaded_at',
    aliasedName,
    true,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [
    ownerUid,
    roomId,
    id,
    sourceRelativePath,
    payloadJson,
    availabilityStatus,
    freshnessStatus,
    localDownloadPath,
    sha256,
    sizeBytes,
    lastVerifiedAt,
    downloadedAt,
    updatedAt,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'cached_smart_cache_files';
  @override
  VerificationContext validateIntegrity(
    Insertable<CachedSmartCacheFile> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('owner_uid')) {
      context.handle(
        _ownerUidMeta,
        ownerUid.isAcceptableOrUnknown(data['owner_uid']!, _ownerUidMeta),
      );
    } else if (isInserting) {
      context.missing(_ownerUidMeta);
    }
    if (data.containsKey('room_id')) {
      context.handle(
        _roomIdMeta,
        roomId.isAcceptableOrUnknown(data['room_id']!, _roomIdMeta),
      );
    } else if (isInserting) {
      context.missing(_roomIdMeta);
    }
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    } else if (isInserting) {
      context.missing(_idMeta);
    }
    if (data.containsKey('source_relative_path')) {
      context.handle(
        _sourceRelativePathMeta,
        sourceRelativePath.isAcceptableOrUnknown(
          data['source_relative_path']!,
          _sourceRelativePathMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_sourceRelativePathMeta);
    }
    if (data.containsKey('payload_json')) {
      context.handle(
        _payloadJsonMeta,
        payloadJson.isAcceptableOrUnknown(
          data['payload_json']!,
          _payloadJsonMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_payloadJsonMeta);
    }
    if (data.containsKey('availability_status')) {
      context.handle(
        _availabilityStatusMeta,
        availabilityStatus.isAcceptableOrUnknown(
          data['availability_status']!,
          _availabilityStatusMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_availabilityStatusMeta);
    }
    if (data.containsKey('freshness_status')) {
      context.handle(
        _freshnessStatusMeta,
        freshnessStatus.isAcceptableOrUnknown(
          data['freshness_status']!,
          _freshnessStatusMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_freshnessStatusMeta);
    }
    if (data.containsKey('local_download_path')) {
      context.handle(
        _localDownloadPathMeta,
        localDownloadPath.isAcceptableOrUnknown(
          data['local_download_path']!,
          _localDownloadPathMeta,
        ),
      );
    }
    if (data.containsKey('sha256')) {
      context.handle(
        _sha256Meta,
        sha256.isAcceptableOrUnknown(data['sha256']!, _sha256Meta),
      );
    }
    if (data.containsKey('size_bytes')) {
      context.handle(
        _sizeBytesMeta,
        sizeBytes.isAcceptableOrUnknown(data['size_bytes']!, _sizeBytesMeta),
      );
    }
    if (data.containsKey('last_verified_at')) {
      context.handle(
        _lastVerifiedAtMeta,
        lastVerifiedAt.isAcceptableOrUnknown(
          data['last_verified_at']!,
          _lastVerifiedAtMeta,
        ),
      );
    }
    if (data.containsKey('downloaded_at')) {
      context.handle(
        _downloadedAtMeta,
        downloadedAt.isAcceptableOrUnknown(
          data['downloaded_at']!,
          _downloadedAtMeta,
        ),
      );
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {ownerUid, id};
  @override
  CachedSmartCacheFile map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return CachedSmartCacheFile(
      ownerUid: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}owner_uid'],
      )!,
      roomId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}room_id'],
      )!,
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}id'],
      )!,
      sourceRelativePath: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}source_relative_path'],
      )!,
      payloadJson: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}payload_json'],
      )!,
      availabilityStatus: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}availability_status'],
      )!,
      freshnessStatus: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}freshness_status'],
      )!,
      localDownloadPath: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}local_download_path'],
      ),
      sha256: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}sha256'],
      ),
      sizeBytes: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}size_bytes'],
      ),
      lastVerifiedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}last_verified_at'],
      ),
      downloadedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}downloaded_at'],
      ),
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $CachedSmartCacheFilesTable createAlias(String alias) {
    return $CachedSmartCacheFilesTable(attachedDatabase, alias);
  }
}

class CachedSmartCacheFile extends DataClass
    implements Insertable<CachedSmartCacheFile> {
  final String ownerUid;
  final String roomId;
  final String id;
  final String sourceRelativePath;
  final String payloadJson;
  final String availabilityStatus;
  final String freshnessStatus;
  final String? localDownloadPath;
  final String? sha256;
  final int? sizeBytes;
  final DateTime? lastVerifiedAt;
  final DateTime? downloadedAt;
  final DateTime updatedAt;
  const CachedSmartCacheFile({
    required this.ownerUid,
    required this.roomId,
    required this.id,
    required this.sourceRelativePath,
    required this.payloadJson,
    required this.availabilityStatus,
    required this.freshnessStatus,
    this.localDownloadPath,
    this.sha256,
    this.sizeBytes,
    this.lastVerifiedAt,
    this.downloadedAt,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['owner_uid'] = Variable<String>(ownerUid);
    map['room_id'] = Variable<String>(roomId);
    map['id'] = Variable<String>(id);
    map['source_relative_path'] = Variable<String>(sourceRelativePath);
    map['payload_json'] = Variable<String>(payloadJson);
    map['availability_status'] = Variable<String>(availabilityStatus);
    map['freshness_status'] = Variable<String>(freshnessStatus);
    if (!nullToAbsent || localDownloadPath != null) {
      map['local_download_path'] = Variable<String>(localDownloadPath);
    }
    if (!nullToAbsent || sha256 != null) {
      map['sha256'] = Variable<String>(sha256);
    }
    if (!nullToAbsent || sizeBytes != null) {
      map['size_bytes'] = Variable<int>(sizeBytes);
    }
    if (!nullToAbsent || lastVerifiedAt != null) {
      map['last_verified_at'] = Variable<DateTime>(lastVerifiedAt);
    }
    if (!nullToAbsent || downloadedAt != null) {
      map['downloaded_at'] = Variable<DateTime>(downloadedAt);
    }
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  CachedSmartCacheFilesCompanion toCompanion(bool nullToAbsent) {
    return CachedSmartCacheFilesCompanion(
      ownerUid: Value(ownerUid),
      roomId: Value(roomId),
      id: Value(id),
      sourceRelativePath: Value(sourceRelativePath),
      payloadJson: Value(payloadJson),
      availabilityStatus: Value(availabilityStatus),
      freshnessStatus: Value(freshnessStatus),
      localDownloadPath: localDownloadPath == null && nullToAbsent
          ? const Value.absent()
          : Value(localDownloadPath),
      sha256: sha256 == null && nullToAbsent
          ? const Value.absent()
          : Value(sha256),
      sizeBytes: sizeBytes == null && nullToAbsent
          ? const Value.absent()
          : Value(sizeBytes),
      lastVerifiedAt: lastVerifiedAt == null && nullToAbsent
          ? const Value.absent()
          : Value(lastVerifiedAt),
      downloadedAt: downloadedAt == null && nullToAbsent
          ? const Value.absent()
          : Value(downloadedAt),
      updatedAt: Value(updatedAt),
    );
  }

  factory CachedSmartCacheFile.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return CachedSmartCacheFile(
      ownerUid: serializer.fromJson<String>(json['ownerUid']),
      roomId: serializer.fromJson<String>(json['roomId']),
      id: serializer.fromJson<String>(json['id']),
      sourceRelativePath: serializer.fromJson<String>(
        json['sourceRelativePath'],
      ),
      payloadJson: serializer.fromJson<String>(json['payloadJson']),
      availabilityStatus: serializer.fromJson<String>(
        json['availabilityStatus'],
      ),
      freshnessStatus: serializer.fromJson<String>(json['freshnessStatus']),
      localDownloadPath: serializer.fromJson<String?>(
        json['localDownloadPath'],
      ),
      sha256: serializer.fromJson<String?>(json['sha256']),
      sizeBytes: serializer.fromJson<int?>(json['sizeBytes']),
      lastVerifiedAt: serializer.fromJson<DateTime?>(json['lastVerifiedAt']),
      downloadedAt: serializer.fromJson<DateTime?>(json['downloadedAt']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'ownerUid': serializer.toJson<String>(ownerUid),
      'roomId': serializer.toJson<String>(roomId),
      'id': serializer.toJson<String>(id),
      'sourceRelativePath': serializer.toJson<String>(sourceRelativePath),
      'payloadJson': serializer.toJson<String>(payloadJson),
      'availabilityStatus': serializer.toJson<String>(availabilityStatus),
      'freshnessStatus': serializer.toJson<String>(freshnessStatus),
      'localDownloadPath': serializer.toJson<String?>(localDownloadPath),
      'sha256': serializer.toJson<String?>(sha256),
      'sizeBytes': serializer.toJson<int?>(sizeBytes),
      'lastVerifiedAt': serializer.toJson<DateTime?>(lastVerifiedAt),
      'downloadedAt': serializer.toJson<DateTime?>(downloadedAt),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  CachedSmartCacheFile copyWith({
    String? ownerUid,
    String? roomId,
    String? id,
    String? sourceRelativePath,
    String? payloadJson,
    String? availabilityStatus,
    String? freshnessStatus,
    Value<String?> localDownloadPath = const Value.absent(),
    Value<String?> sha256 = const Value.absent(),
    Value<int?> sizeBytes = const Value.absent(),
    Value<DateTime?> lastVerifiedAt = const Value.absent(),
    Value<DateTime?> downloadedAt = const Value.absent(),
    DateTime? updatedAt,
  }) => CachedSmartCacheFile(
    ownerUid: ownerUid ?? this.ownerUid,
    roomId: roomId ?? this.roomId,
    id: id ?? this.id,
    sourceRelativePath: sourceRelativePath ?? this.sourceRelativePath,
    payloadJson: payloadJson ?? this.payloadJson,
    availabilityStatus: availabilityStatus ?? this.availabilityStatus,
    freshnessStatus: freshnessStatus ?? this.freshnessStatus,
    localDownloadPath: localDownloadPath.present
        ? localDownloadPath.value
        : this.localDownloadPath,
    sha256: sha256.present ? sha256.value : this.sha256,
    sizeBytes: sizeBytes.present ? sizeBytes.value : this.sizeBytes,
    lastVerifiedAt: lastVerifiedAt.present
        ? lastVerifiedAt.value
        : this.lastVerifiedAt,
    downloadedAt: downloadedAt.present ? downloadedAt.value : this.downloadedAt,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  CachedSmartCacheFile copyWithCompanion(CachedSmartCacheFilesCompanion data) {
    return CachedSmartCacheFile(
      ownerUid: data.ownerUid.present ? data.ownerUid.value : this.ownerUid,
      roomId: data.roomId.present ? data.roomId.value : this.roomId,
      id: data.id.present ? data.id.value : this.id,
      sourceRelativePath: data.sourceRelativePath.present
          ? data.sourceRelativePath.value
          : this.sourceRelativePath,
      payloadJson: data.payloadJson.present
          ? data.payloadJson.value
          : this.payloadJson,
      availabilityStatus: data.availabilityStatus.present
          ? data.availabilityStatus.value
          : this.availabilityStatus,
      freshnessStatus: data.freshnessStatus.present
          ? data.freshnessStatus.value
          : this.freshnessStatus,
      localDownloadPath: data.localDownloadPath.present
          ? data.localDownloadPath.value
          : this.localDownloadPath,
      sha256: data.sha256.present ? data.sha256.value : this.sha256,
      sizeBytes: data.sizeBytes.present ? data.sizeBytes.value : this.sizeBytes,
      lastVerifiedAt: data.lastVerifiedAt.present
          ? data.lastVerifiedAt.value
          : this.lastVerifiedAt,
      downloadedAt: data.downloadedAt.present
          ? data.downloadedAt.value
          : this.downloadedAt,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('CachedSmartCacheFile(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('id: $id, ')
          ..write('sourceRelativePath: $sourceRelativePath, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('availabilityStatus: $availabilityStatus, ')
          ..write('freshnessStatus: $freshnessStatus, ')
          ..write('localDownloadPath: $localDownloadPath, ')
          ..write('sha256: $sha256, ')
          ..write('sizeBytes: $sizeBytes, ')
          ..write('lastVerifiedAt: $lastVerifiedAt, ')
          ..write('downloadedAt: $downloadedAt, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
    ownerUid,
    roomId,
    id,
    sourceRelativePath,
    payloadJson,
    availabilityStatus,
    freshnessStatus,
    localDownloadPath,
    sha256,
    sizeBytes,
    lastVerifiedAt,
    downloadedAt,
    updatedAt,
  );
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CachedSmartCacheFile &&
          other.ownerUid == this.ownerUid &&
          other.roomId == this.roomId &&
          other.id == this.id &&
          other.sourceRelativePath == this.sourceRelativePath &&
          other.payloadJson == this.payloadJson &&
          other.availabilityStatus == this.availabilityStatus &&
          other.freshnessStatus == this.freshnessStatus &&
          other.localDownloadPath == this.localDownloadPath &&
          other.sha256 == this.sha256 &&
          other.sizeBytes == this.sizeBytes &&
          other.lastVerifiedAt == this.lastVerifiedAt &&
          other.downloadedAt == this.downloadedAt &&
          other.updatedAt == this.updatedAt);
}

class CachedSmartCacheFilesCompanion
    extends UpdateCompanion<CachedSmartCacheFile> {
  final Value<String> ownerUid;
  final Value<String> roomId;
  final Value<String> id;
  final Value<String> sourceRelativePath;
  final Value<String> payloadJson;
  final Value<String> availabilityStatus;
  final Value<String> freshnessStatus;
  final Value<String?> localDownloadPath;
  final Value<String?> sha256;
  final Value<int?> sizeBytes;
  final Value<DateTime?> lastVerifiedAt;
  final Value<DateTime?> downloadedAt;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const CachedSmartCacheFilesCompanion({
    this.ownerUid = const Value.absent(),
    this.roomId = const Value.absent(),
    this.id = const Value.absent(),
    this.sourceRelativePath = const Value.absent(),
    this.payloadJson = const Value.absent(),
    this.availabilityStatus = const Value.absent(),
    this.freshnessStatus = const Value.absent(),
    this.localDownloadPath = const Value.absent(),
    this.sha256 = const Value.absent(),
    this.sizeBytes = const Value.absent(),
    this.lastVerifiedAt = const Value.absent(),
    this.downloadedAt = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  CachedSmartCacheFilesCompanion.insert({
    required String ownerUid,
    required String roomId,
    required String id,
    required String sourceRelativePath,
    required String payloadJson,
    required String availabilityStatus,
    required String freshnessStatus,
    this.localDownloadPath = const Value.absent(),
    this.sha256 = const Value.absent(),
    this.sizeBytes = const Value.absent(),
    this.lastVerifiedAt = const Value.absent(),
    this.downloadedAt = const Value.absent(),
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : ownerUid = Value(ownerUid),
       roomId = Value(roomId),
       id = Value(id),
       sourceRelativePath = Value(sourceRelativePath),
       payloadJson = Value(payloadJson),
       availabilityStatus = Value(availabilityStatus),
       freshnessStatus = Value(freshnessStatus),
       updatedAt = Value(updatedAt);
  static Insertable<CachedSmartCacheFile> custom({
    Expression<String>? ownerUid,
    Expression<String>? roomId,
    Expression<String>? id,
    Expression<String>? sourceRelativePath,
    Expression<String>? payloadJson,
    Expression<String>? availabilityStatus,
    Expression<String>? freshnessStatus,
    Expression<String>? localDownloadPath,
    Expression<String>? sha256,
    Expression<int>? sizeBytes,
    Expression<DateTime>? lastVerifiedAt,
    Expression<DateTime>? downloadedAt,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (ownerUid != null) 'owner_uid': ownerUid,
      if (roomId != null) 'room_id': roomId,
      if (id != null) 'id': id,
      if (sourceRelativePath != null)
        'source_relative_path': sourceRelativePath,
      if (payloadJson != null) 'payload_json': payloadJson,
      if (availabilityStatus != null) 'availability_status': availabilityStatus,
      if (freshnessStatus != null) 'freshness_status': freshnessStatus,
      if (localDownloadPath != null) 'local_download_path': localDownloadPath,
      if (sha256 != null) 'sha256': sha256,
      if (sizeBytes != null) 'size_bytes': sizeBytes,
      if (lastVerifiedAt != null) 'last_verified_at': lastVerifiedAt,
      if (downloadedAt != null) 'downloaded_at': downloadedAt,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  CachedSmartCacheFilesCompanion copyWith({
    Value<String>? ownerUid,
    Value<String>? roomId,
    Value<String>? id,
    Value<String>? sourceRelativePath,
    Value<String>? payloadJson,
    Value<String>? availabilityStatus,
    Value<String>? freshnessStatus,
    Value<String?>? localDownloadPath,
    Value<String?>? sha256,
    Value<int?>? sizeBytes,
    Value<DateTime?>? lastVerifiedAt,
    Value<DateTime?>? downloadedAt,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return CachedSmartCacheFilesCompanion(
      ownerUid: ownerUid ?? this.ownerUid,
      roomId: roomId ?? this.roomId,
      id: id ?? this.id,
      sourceRelativePath: sourceRelativePath ?? this.sourceRelativePath,
      payloadJson: payloadJson ?? this.payloadJson,
      availabilityStatus: availabilityStatus ?? this.availabilityStatus,
      freshnessStatus: freshnessStatus ?? this.freshnessStatus,
      localDownloadPath: localDownloadPath ?? this.localDownloadPath,
      sha256: sha256 ?? this.sha256,
      sizeBytes: sizeBytes ?? this.sizeBytes,
      lastVerifiedAt: lastVerifiedAt ?? this.lastVerifiedAt,
      downloadedAt: downloadedAt ?? this.downloadedAt,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (ownerUid.present) {
      map['owner_uid'] = Variable<String>(ownerUid.value);
    }
    if (roomId.present) {
      map['room_id'] = Variable<String>(roomId.value);
    }
    if (id.present) {
      map['id'] = Variable<String>(id.value);
    }
    if (sourceRelativePath.present) {
      map['source_relative_path'] = Variable<String>(sourceRelativePath.value);
    }
    if (payloadJson.present) {
      map['payload_json'] = Variable<String>(payloadJson.value);
    }
    if (availabilityStatus.present) {
      map['availability_status'] = Variable<String>(availabilityStatus.value);
    }
    if (freshnessStatus.present) {
      map['freshness_status'] = Variable<String>(freshnessStatus.value);
    }
    if (localDownloadPath.present) {
      map['local_download_path'] = Variable<String>(localDownloadPath.value);
    }
    if (sha256.present) {
      map['sha256'] = Variable<String>(sha256.value);
    }
    if (sizeBytes.present) {
      map['size_bytes'] = Variable<int>(sizeBytes.value);
    }
    if (lastVerifiedAt.present) {
      map['last_verified_at'] = Variable<DateTime>(lastVerifiedAt.value);
    }
    if (downloadedAt.present) {
      map['downloaded_at'] = Variable<DateTime>(downloadedAt.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('CachedSmartCacheFilesCompanion(')
          ..write('ownerUid: $ownerUid, ')
          ..write('roomId: $roomId, ')
          ..write('id: $id, ')
          ..write('sourceRelativePath: $sourceRelativePath, ')
          ..write('payloadJson: $payloadJson, ')
          ..write('availabilityStatus: $availabilityStatus, ')
          ..write('freshnessStatus: $freshnessStatus, ')
          ..write('localDownloadPath: $localDownloadPath, ')
          ..write('sha256: $sha256, ')
          ..write('sizeBytes: $sizeBytes, ')
          ..write('lastVerifiedAt: $lastVerifiedAt, ')
          ..write('downloadedAt: $downloadedAt, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

abstract class _$AppDatabase extends GeneratedDatabase {
  _$AppDatabase(QueryExecutor e) : super(e);
  $AppDatabaseManager get managers => $AppDatabaseManager(this);
  late final $CachedDevicesTable cachedDevices = $CachedDevicesTable(this);
  late final $CachedRoomsTable cachedRooms = $CachedRoomsTable(this);
  late final $CachedCommandsTable cachedCommands = $CachedCommandsTable(this);
  late final $CachedProposalsTable cachedProposals = $CachedProposalsTable(
    this,
  );
  late final $CachedExecutionsTable cachedExecutions = $CachedExecutionsTable(
    this,
  );
  late final $CachedRoomSnapshotsTable cachedRoomSnapshots =
      $CachedRoomSnapshotsTable(this);
  late final $MutationOutboxTable mutationOutbox = $MutationOutboxTable(this);
  late final $SyncCursorsTable syncCursors = $SyncCursorsTable(this);
  late final $CachedSmartCacheFilesTable cachedSmartCacheFiles =
      $CachedSmartCacheFilesTable(this);
  @override
  Iterable<TableInfo<Table, Object?>> get allTables =>
      allSchemaEntities.whereType<TableInfo<Table, Object?>>();
  @override
  List<DatabaseSchemaEntity> get allSchemaEntities => [
    cachedDevices,
    cachedRooms,
    cachedCommands,
    cachedProposals,
    cachedExecutions,
    cachedRoomSnapshots,
    mutationOutbox,
    syncCursors,
    cachedSmartCacheFiles,
  ];
}

typedef $$CachedDevicesTableCreateCompanionBuilder =
    CachedDevicesCompanion Function({
      required String ownerUid,
      required String id,
      required String payloadJson,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$CachedDevicesTableUpdateCompanionBuilder =
    CachedDevicesCompanion Function({
      Value<String> ownerUid,
      Value<String> id,
      Value<String> payloadJson,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$CachedDevicesTableFilterComposer
    extends Composer<_$AppDatabase, $CachedDevicesTable> {
  $$CachedDevicesTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$CachedDevicesTableOrderingComposer
    extends Composer<_$AppDatabase, $CachedDevicesTable> {
  $$CachedDevicesTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$CachedDevicesTableAnnotationComposer
    extends Composer<_$AppDatabase, $CachedDevicesTable> {
  $$CachedDevicesTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$CachedDevicesTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $CachedDevicesTable,
          CachedDevice,
          $$CachedDevicesTableFilterComposer,
          $$CachedDevicesTableOrderingComposer,
          $$CachedDevicesTableAnnotationComposer,
          $$CachedDevicesTableCreateCompanionBuilder,
          $$CachedDevicesTableUpdateCompanionBuilder,
          (
            CachedDevice,
            BaseReferences<_$AppDatabase, $CachedDevicesTable, CachedDevice>,
          ),
          CachedDevice,
          PrefetchHooks Function()
        > {
  $$CachedDevicesTableTableManager(_$AppDatabase db, $CachedDevicesTable table)
    : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$CachedDevicesTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$CachedDevicesTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$CachedDevicesTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> id = const Value.absent(),
                Value<String> payloadJson = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => CachedDevicesCompanion(
                ownerUid: ownerUid,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String id,
                required String payloadJson,
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => CachedDevicesCompanion.insert(
                ownerUid: ownerUid,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$CachedDevicesTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $CachedDevicesTable,
      CachedDevice,
      $$CachedDevicesTableFilterComposer,
      $$CachedDevicesTableOrderingComposer,
      $$CachedDevicesTableAnnotationComposer,
      $$CachedDevicesTableCreateCompanionBuilder,
      $$CachedDevicesTableUpdateCompanionBuilder,
      (
        CachedDevice,
        BaseReferences<_$AppDatabase, $CachedDevicesTable, CachedDevice>,
      ),
      CachedDevice,
      PrefetchHooks Function()
    >;
typedef $$CachedRoomsTableCreateCompanionBuilder =
    CachedRoomsCompanion Function({
      required String ownerUid,
      required String id,
      required String payloadJson,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$CachedRoomsTableUpdateCompanionBuilder =
    CachedRoomsCompanion Function({
      Value<String> ownerUid,
      Value<String> id,
      Value<String> payloadJson,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$CachedRoomsTableFilterComposer
    extends Composer<_$AppDatabase, $CachedRoomsTable> {
  $$CachedRoomsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$CachedRoomsTableOrderingComposer
    extends Composer<_$AppDatabase, $CachedRoomsTable> {
  $$CachedRoomsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$CachedRoomsTableAnnotationComposer
    extends Composer<_$AppDatabase, $CachedRoomsTable> {
  $$CachedRoomsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$CachedRoomsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $CachedRoomsTable,
          CachedRoom,
          $$CachedRoomsTableFilterComposer,
          $$CachedRoomsTableOrderingComposer,
          $$CachedRoomsTableAnnotationComposer,
          $$CachedRoomsTableCreateCompanionBuilder,
          $$CachedRoomsTableUpdateCompanionBuilder,
          (
            CachedRoom,
            BaseReferences<_$AppDatabase, $CachedRoomsTable, CachedRoom>,
          ),
          CachedRoom,
          PrefetchHooks Function()
        > {
  $$CachedRoomsTableTableManager(_$AppDatabase db, $CachedRoomsTable table)
    : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$CachedRoomsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$CachedRoomsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$CachedRoomsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> id = const Value.absent(),
                Value<String> payloadJson = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => CachedRoomsCompanion(
                ownerUid: ownerUid,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String id,
                required String payloadJson,
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => CachedRoomsCompanion.insert(
                ownerUid: ownerUid,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$CachedRoomsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $CachedRoomsTable,
      CachedRoom,
      $$CachedRoomsTableFilterComposer,
      $$CachedRoomsTableOrderingComposer,
      $$CachedRoomsTableAnnotationComposer,
      $$CachedRoomsTableCreateCompanionBuilder,
      $$CachedRoomsTableUpdateCompanionBuilder,
      (
        CachedRoom,
        BaseReferences<_$AppDatabase, $CachedRoomsTable, CachedRoom>,
      ),
      CachedRoom,
      PrefetchHooks Function()
    >;
typedef $$CachedCommandsTableCreateCompanionBuilder =
    CachedCommandsCompanion Function({
      required String ownerUid,
      required String roomId,
      required String id,
      required String payloadJson,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$CachedCommandsTableUpdateCompanionBuilder =
    CachedCommandsCompanion Function({
      Value<String> ownerUid,
      Value<String> roomId,
      Value<String> id,
      Value<String> payloadJson,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$CachedCommandsTableFilterComposer
    extends Composer<_$AppDatabase, $CachedCommandsTable> {
  $$CachedCommandsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$CachedCommandsTableOrderingComposer
    extends Composer<_$AppDatabase, $CachedCommandsTable> {
  $$CachedCommandsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$CachedCommandsTableAnnotationComposer
    extends Composer<_$AppDatabase, $CachedCommandsTable> {
  $$CachedCommandsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get roomId =>
      $composableBuilder(column: $table.roomId, builder: (column) => column);

  GeneratedColumn<String> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$CachedCommandsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $CachedCommandsTable,
          CachedCommand,
          $$CachedCommandsTableFilterComposer,
          $$CachedCommandsTableOrderingComposer,
          $$CachedCommandsTableAnnotationComposer,
          $$CachedCommandsTableCreateCompanionBuilder,
          $$CachedCommandsTableUpdateCompanionBuilder,
          (
            CachedCommand,
            BaseReferences<_$AppDatabase, $CachedCommandsTable, CachedCommand>,
          ),
          CachedCommand,
          PrefetchHooks Function()
        > {
  $$CachedCommandsTableTableManager(
    _$AppDatabase db,
    $CachedCommandsTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$CachedCommandsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$CachedCommandsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$CachedCommandsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> roomId = const Value.absent(),
                Value<String> id = const Value.absent(),
                Value<String> payloadJson = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => CachedCommandsCompanion(
                ownerUid: ownerUid,
                roomId: roomId,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String roomId,
                required String id,
                required String payloadJson,
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => CachedCommandsCompanion.insert(
                ownerUid: ownerUid,
                roomId: roomId,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$CachedCommandsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $CachedCommandsTable,
      CachedCommand,
      $$CachedCommandsTableFilterComposer,
      $$CachedCommandsTableOrderingComposer,
      $$CachedCommandsTableAnnotationComposer,
      $$CachedCommandsTableCreateCompanionBuilder,
      $$CachedCommandsTableUpdateCompanionBuilder,
      (
        CachedCommand,
        BaseReferences<_$AppDatabase, $CachedCommandsTable, CachedCommand>,
      ),
      CachedCommand,
      PrefetchHooks Function()
    >;
typedef $$CachedProposalsTableCreateCompanionBuilder =
    CachedProposalsCompanion Function({
      required String ownerUid,
      required String roomId,
      required String id,
      required String payloadJson,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$CachedProposalsTableUpdateCompanionBuilder =
    CachedProposalsCompanion Function({
      Value<String> ownerUid,
      Value<String> roomId,
      Value<String> id,
      Value<String> payloadJson,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$CachedProposalsTableFilterComposer
    extends Composer<_$AppDatabase, $CachedProposalsTable> {
  $$CachedProposalsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$CachedProposalsTableOrderingComposer
    extends Composer<_$AppDatabase, $CachedProposalsTable> {
  $$CachedProposalsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$CachedProposalsTableAnnotationComposer
    extends Composer<_$AppDatabase, $CachedProposalsTable> {
  $$CachedProposalsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get roomId =>
      $composableBuilder(column: $table.roomId, builder: (column) => column);

  GeneratedColumn<String> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$CachedProposalsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $CachedProposalsTable,
          CachedProposal,
          $$CachedProposalsTableFilterComposer,
          $$CachedProposalsTableOrderingComposer,
          $$CachedProposalsTableAnnotationComposer,
          $$CachedProposalsTableCreateCompanionBuilder,
          $$CachedProposalsTableUpdateCompanionBuilder,
          (
            CachedProposal,
            BaseReferences<
              _$AppDatabase,
              $CachedProposalsTable,
              CachedProposal
            >,
          ),
          CachedProposal,
          PrefetchHooks Function()
        > {
  $$CachedProposalsTableTableManager(
    _$AppDatabase db,
    $CachedProposalsTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$CachedProposalsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$CachedProposalsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$CachedProposalsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> roomId = const Value.absent(),
                Value<String> id = const Value.absent(),
                Value<String> payloadJson = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => CachedProposalsCompanion(
                ownerUid: ownerUid,
                roomId: roomId,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String roomId,
                required String id,
                required String payloadJson,
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => CachedProposalsCompanion.insert(
                ownerUid: ownerUid,
                roomId: roomId,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$CachedProposalsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $CachedProposalsTable,
      CachedProposal,
      $$CachedProposalsTableFilterComposer,
      $$CachedProposalsTableOrderingComposer,
      $$CachedProposalsTableAnnotationComposer,
      $$CachedProposalsTableCreateCompanionBuilder,
      $$CachedProposalsTableUpdateCompanionBuilder,
      (
        CachedProposal,
        BaseReferences<_$AppDatabase, $CachedProposalsTable, CachedProposal>,
      ),
      CachedProposal,
      PrefetchHooks Function()
    >;
typedef $$CachedExecutionsTableCreateCompanionBuilder =
    CachedExecutionsCompanion Function({
      required String ownerUid,
      required String roomId,
      required String id,
      required String payloadJson,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$CachedExecutionsTableUpdateCompanionBuilder =
    CachedExecutionsCompanion Function({
      Value<String> ownerUid,
      Value<String> roomId,
      Value<String> id,
      Value<String> payloadJson,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$CachedExecutionsTableFilterComposer
    extends Composer<_$AppDatabase, $CachedExecutionsTable> {
  $$CachedExecutionsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$CachedExecutionsTableOrderingComposer
    extends Composer<_$AppDatabase, $CachedExecutionsTable> {
  $$CachedExecutionsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$CachedExecutionsTableAnnotationComposer
    extends Composer<_$AppDatabase, $CachedExecutionsTable> {
  $$CachedExecutionsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get roomId =>
      $composableBuilder(column: $table.roomId, builder: (column) => column);

  GeneratedColumn<String> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$CachedExecutionsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $CachedExecutionsTable,
          CachedExecution,
          $$CachedExecutionsTableFilterComposer,
          $$CachedExecutionsTableOrderingComposer,
          $$CachedExecutionsTableAnnotationComposer,
          $$CachedExecutionsTableCreateCompanionBuilder,
          $$CachedExecutionsTableUpdateCompanionBuilder,
          (
            CachedExecution,
            BaseReferences<
              _$AppDatabase,
              $CachedExecutionsTable,
              CachedExecution
            >,
          ),
          CachedExecution,
          PrefetchHooks Function()
        > {
  $$CachedExecutionsTableTableManager(
    _$AppDatabase db,
    $CachedExecutionsTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$CachedExecutionsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$CachedExecutionsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$CachedExecutionsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> roomId = const Value.absent(),
                Value<String> id = const Value.absent(),
                Value<String> payloadJson = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => CachedExecutionsCompanion(
                ownerUid: ownerUid,
                roomId: roomId,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String roomId,
                required String id,
                required String payloadJson,
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => CachedExecutionsCompanion.insert(
                ownerUid: ownerUid,
                roomId: roomId,
                id: id,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$CachedExecutionsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $CachedExecutionsTable,
      CachedExecution,
      $$CachedExecutionsTableFilterComposer,
      $$CachedExecutionsTableOrderingComposer,
      $$CachedExecutionsTableAnnotationComposer,
      $$CachedExecutionsTableCreateCompanionBuilder,
      $$CachedExecutionsTableUpdateCompanionBuilder,
      (
        CachedExecution,
        BaseReferences<_$AppDatabase, $CachedExecutionsTable, CachedExecution>,
      ),
      CachedExecution,
      PrefetchHooks Function()
    >;
typedef $$CachedRoomSnapshotsTableCreateCompanionBuilder =
    CachedRoomSnapshotsCompanion Function({
      required String ownerUid,
      required String roomId,
      required String payloadJson,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$CachedRoomSnapshotsTableUpdateCompanionBuilder =
    CachedRoomSnapshotsCompanion Function({
      Value<String> ownerUid,
      Value<String> roomId,
      Value<String> payloadJson,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$CachedRoomSnapshotsTableFilterComposer
    extends Composer<_$AppDatabase, $CachedRoomSnapshotsTable> {
  $$CachedRoomSnapshotsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$CachedRoomSnapshotsTableOrderingComposer
    extends Composer<_$AppDatabase, $CachedRoomSnapshotsTable> {
  $$CachedRoomSnapshotsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$CachedRoomSnapshotsTableAnnotationComposer
    extends Composer<_$AppDatabase, $CachedRoomSnapshotsTable> {
  $$CachedRoomSnapshotsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get roomId =>
      $composableBuilder(column: $table.roomId, builder: (column) => column);

  GeneratedColumn<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$CachedRoomSnapshotsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $CachedRoomSnapshotsTable,
          CachedRoomSnapshot,
          $$CachedRoomSnapshotsTableFilterComposer,
          $$CachedRoomSnapshotsTableOrderingComposer,
          $$CachedRoomSnapshotsTableAnnotationComposer,
          $$CachedRoomSnapshotsTableCreateCompanionBuilder,
          $$CachedRoomSnapshotsTableUpdateCompanionBuilder,
          (
            CachedRoomSnapshot,
            BaseReferences<
              _$AppDatabase,
              $CachedRoomSnapshotsTable,
              CachedRoomSnapshot
            >,
          ),
          CachedRoomSnapshot,
          PrefetchHooks Function()
        > {
  $$CachedRoomSnapshotsTableTableManager(
    _$AppDatabase db,
    $CachedRoomSnapshotsTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$CachedRoomSnapshotsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$CachedRoomSnapshotsTableOrderingComposer(
                $db: db,
                $table: table,
              ),
          createComputedFieldComposer: () =>
              $$CachedRoomSnapshotsTableAnnotationComposer(
                $db: db,
                $table: table,
              ),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> roomId = const Value.absent(),
                Value<String> payloadJson = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => CachedRoomSnapshotsCompanion(
                ownerUid: ownerUid,
                roomId: roomId,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String roomId,
                required String payloadJson,
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => CachedRoomSnapshotsCompanion.insert(
                ownerUid: ownerUid,
                roomId: roomId,
                payloadJson: payloadJson,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$CachedRoomSnapshotsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $CachedRoomSnapshotsTable,
      CachedRoomSnapshot,
      $$CachedRoomSnapshotsTableFilterComposer,
      $$CachedRoomSnapshotsTableOrderingComposer,
      $$CachedRoomSnapshotsTableAnnotationComposer,
      $$CachedRoomSnapshotsTableCreateCompanionBuilder,
      $$CachedRoomSnapshotsTableUpdateCompanionBuilder,
      (
        CachedRoomSnapshot,
        BaseReferences<
          _$AppDatabase,
          $CachedRoomSnapshotsTable,
          CachedRoomSnapshot
        >,
      ),
      CachedRoomSnapshot,
      PrefetchHooks Function()
    >;
typedef $$MutationOutboxTableCreateCompanionBuilder =
    MutationOutboxCompanion Function({
      required String ownerUid,
      required String id,
      required String mutationType,
      required String payloadJson,
      Value<int> attemptCount,
      required DateTime nextRetryAt,
      required DateTime createdAt,
      Value<String> status,
      Value<String?> lastErrorCode,
      Value<int> rowid,
    });
typedef $$MutationOutboxTableUpdateCompanionBuilder =
    MutationOutboxCompanion Function({
      Value<String> ownerUid,
      Value<String> id,
      Value<String> mutationType,
      Value<String> payloadJson,
      Value<int> attemptCount,
      Value<DateTime> nextRetryAt,
      Value<DateTime> createdAt,
      Value<String> status,
      Value<String?> lastErrorCode,
      Value<int> rowid,
    });

class $$MutationOutboxTableFilterComposer
    extends Composer<_$AppDatabase, $MutationOutboxTable> {
  $$MutationOutboxTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get mutationType => $composableBuilder(
    column: $table.mutationType,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get attemptCount => $composableBuilder(
    column: $table.attemptCount,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get nextRetryAt => $composableBuilder(
    column: $table.nextRetryAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get createdAt => $composableBuilder(
    column: $table.createdAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get status => $composableBuilder(
    column: $table.status,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get lastErrorCode => $composableBuilder(
    column: $table.lastErrorCode,
    builder: (column) => ColumnFilters(column),
  );
}

class $$MutationOutboxTableOrderingComposer
    extends Composer<_$AppDatabase, $MutationOutboxTable> {
  $$MutationOutboxTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get mutationType => $composableBuilder(
    column: $table.mutationType,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get attemptCount => $composableBuilder(
    column: $table.attemptCount,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get nextRetryAt => $composableBuilder(
    column: $table.nextRetryAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get createdAt => $composableBuilder(
    column: $table.createdAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get status => $composableBuilder(
    column: $table.status,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get lastErrorCode => $composableBuilder(
    column: $table.lastErrorCode,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$MutationOutboxTableAnnotationComposer
    extends Composer<_$AppDatabase, $MutationOutboxTable> {
  $$MutationOutboxTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get mutationType => $composableBuilder(
    column: $table.mutationType,
    builder: (column) => column,
  );

  GeneratedColumn<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => column,
  );

  GeneratedColumn<int> get attemptCount => $composableBuilder(
    column: $table.attemptCount,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get nextRetryAt => $composableBuilder(
    column: $table.nextRetryAt,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get createdAt =>
      $composableBuilder(column: $table.createdAt, builder: (column) => column);

  GeneratedColumn<String> get status =>
      $composableBuilder(column: $table.status, builder: (column) => column);

  GeneratedColumn<String> get lastErrorCode => $composableBuilder(
    column: $table.lastErrorCode,
    builder: (column) => column,
  );
}

class $$MutationOutboxTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $MutationOutboxTable,
          MutationOutboxData,
          $$MutationOutboxTableFilterComposer,
          $$MutationOutboxTableOrderingComposer,
          $$MutationOutboxTableAnnotationComposer,
          $$MutationOutboxTableCreateCompanionBuilder,
          $$MutationOutboxTableUpdateCompanionBuilder,
          (
            MutationOutboxData,
            BaseReferences<
              _$AppDatabase,
              $MutationOutboxTable,
              MutationOutboxData
            >,
          ),
          MutationOutboxData,
          PrefetchHooks Function()
        > {
  $$MutationOutboxTableTableManager(
    _$AppDatabase db,
    $MutationOutboxTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$MutationOutboxTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$MutationOutboxTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$MutationOutboxTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> id = const Value.absent(),
                Value<String> mutationType = const Value.absent(),
                Value<String> payloadJson = const Value.absent(),
                Value<int> attemptCount = const Value.absent(),
                Value<DateTime> nextRetryAt = const Value.absent(),
                Value<DateTime> createdAt = const Value.absent(),
                Value<String> status = const Value.absent(),
                Value<String?> lastErrorCode = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => MutationOutboxCompanion(
                ownerUid: ownerUid,
                id: id,
                mutationType: mutationType,
                payloadJson: payloadJson,
                attemptCount: attemptCount,
                nextRetryAt: nextRetryAt,
                createdAt: createdAt,
                status: status,
                lastErrorCode: lastErrorCode,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String id,
                required String mutationType,
                required String payloadJson,
                Value<int> attemptCount = const Value.absent(),
                required DateTime nextRetryAt,
                required DateTime createdAt,
                Value<String> status = const Value.absent(),
                Value<String?> lastErrorCode = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => MutationOutboxCompanion.insert(
                ownerUid: ownerUid,
                id: id,
                mutationType: mutationType,
                payloadJson: payloadJson,
                attemptCount: attemptCount,
                nextRetryAt: nextRetryAt,
                createdAt: createdAt,
                status: status,
                lastErrorCode: lastErrorCode,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$MutationOutboxTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $MutationOutboxTable,
      MutationOutboxData,
      $$MutationOutboxTableFilterComposer,
      $$MutationOutboxTableOrderingComposer,
      $$MutationOutboxTableAnnotationComposer,
      $$MutationOutboxTableCreateCompanionBuilder,
      $$MutationOutboxTableUpdateCompanionBuilder,
      (
        MutationOutboxData,
        BaseReferences<_$AppDatabase, $MutationOutboxTable, MutationOutboxData>,
      ),
      MutationOutboxData,
      PrefetchHooks Function()
    >;
typedef $$SyncCursorsTableCreateCompanionBuilder =
    SyncCursorsCompanion Function({
      required String ownerUid,
      required String stream,
      Value<int> lastSequence,
      Value<int> rowid,
    });
typedef $$SyncCursorsTableUpdateCompanionBuilder =
    SyncCursorsCompanion Function({
      Value<String> ownerUid,
      Value<String> stream,
      Value<int> lastSequence,
      Value<int> rowid,
    });

class $$SyncCursorsTableFilterComposer
    extends Composer<_$AppDatabase, $SyncCursorsTable> {
  $$SyncCursorsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get stream => $composableBuilder(
    column: $table.stream,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get lastSequence => $composableBuilder(
    column: $table.lastSequence,
    builder: (column) => ColumnFilters(column),
  );
}

class $$SyncCursorsTableOrderingComposer
    extends Composer<_$AppDatabase, $SyncCursorsTable> {
  $$SyncCursorsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get stream => $composableBuilder(
    column: $table.stream,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get lastSequence => $composableBuilder(
    column: $table.lastSequence,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$SyncCursorsTableAnnotationComposer
    extends Composer<_$AppDatabase, $SyncCursorsTable> {
  $$SyncCursorsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get stream =>
      $composableBuilder(column: $table.stream, builder: (column) => column);

  GeneratedColumn<int> get lastSequence => $composableBuilder(
    column: $table.lastSequence,
    builder: (column) => column,
  );
}

class $$SyncCursorsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $SyncCursorsTable,
          SyncCursor,
          $$SyncCursorsTableFilterComposer,
          $$SyncCursorsTableOrderingComposer,
          $$SyncCursorsTableAnnotationComposer,
          $$SyncCursorsTableCreateCompanionBuilder,
          $$SyncCursorsTableUpdateCompanionBuilder,
          (
            SyncCursor,
            BaseReferences<_$AppDatabase, $SyncCursorsTable, SyncCursor>,
          ),
          SyncCursor,
          PrefetchHooks Function()
        > {
  $$SyncCursorsTableTableManager(_$AppDatabase db, $SyncCursorsTable table)
    : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$SyncCursorsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$SyncCursorsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$SyncCursorsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> stream = const Value.absent(),
                Value<int> lastSequence = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => SyncCursorsCompanion(
                ownerUid: ownerUid,
                stream: stream,
                lastSequence: lastSequence,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String stream,
                Value<int> lastSequence = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => SyncCursorsCompanion.insert(
                ownerUid: ownerUid,
                stream: stream,
                lastSequence: lastSequence,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$SyncCursorsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $SyncCursorsTable,
      SyncCursor,
      $$SyncCursorsTableFilterComposer,
      $$SyncCursorsTableOrderingComposer,
      $$SyncCursorsTableAnnotationComposer,
      $$SyncCursorsTableCreateCompanionBuilder,
      $$SyncCursorsTableUpdateCompanionBuilder,
      (
        SyncCursor,
        BaseReferences<_$AppDatabase, $SyncCursorsTable, SyncCursor>,
      ),
      SyncCursor,
      PrefetchHooks Function()
    >;
typedef $$CachedSmartCacheFilesTableCreateCompanionBuilder =
    CachedSmartCacheFilesCompanion Function({
      required String ownerUid,
      required String roomId,
      required String id,
      required String sourceRelativePath,
      required String payloadJson,
      required String availabilityStatus,
      required String freshnessStatus,
      Value<String?> localDownloadPath,
      Value<String?> sha256,
      Value<int?> sizeBytes,
      Value<DateTime?> lastVerifiedAt,
      Value<DateTime?> downloadedAt,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$CachedSmartCacheFilesTableUpdateCompanionBuilder =
    CachedSmartCacheFilesCompanion Function({
      Value<String> ownerUid,
      Value<String> roomId,
      Value<String> id,
      Value<String> sourceRelativePath,
      Value<String> payloadJson,
      Value<String> availabilityStatus,
      Value<String> freshnessStatus,
      Value<String?> localDownloadPath,
      Value<String?> sha256,
      Value<int?> sizeBytes,
      Value<DateTime?> lastVerifiedAt,
      Value<DateTime?> downloadedAt,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$CachedSmartCacheFilesTableFilterComposer
    extends Composer<_$AppDatabase, $CachedSmartCacheFilesTable> {
  $$CachedSmartCacheFilesTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get sourceRelativePath => $composableBuilder(
    column: $table.sourceRelativePath,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get availabilityStatus => $composableBuilder(
    column: $table.availabilityStatus,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get freshnessStatus => $composableBuilder(
    column: $table.freshnessStatus,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get localDownloadPath => $composableBuilder(
    column: $table.localDownloadPath,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get sha256 => $composableBuilder(
    column: $table.sha256,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get sizeBytes => $composableBuilder(
    column: $table.sizeBytes,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get lastVerifiedAt => $composableBuilder(
    column: $table.lastVerifiedAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get downloadedAt => $composableBuilder(
    column: $table.downloadedAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$CachedSmartCacheFilesTableOrderingComposer
    extends Composer<_$AppDatabase, $CachedSmartCacheFilesTable> {
  $$CachedSmartCacheFilesTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get ownerUid => $composableBuilder(
    column: $table.ownerUid,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get roomId => $composableBuilder(
    column: $table.roomId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get sourceRelativePath => $composableBuilder(
    column: $table.sourceRelativePath,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get availabilityStatus => $composableBuilder(
    column: $table.availabilityStatus,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get freshnessStatus => $composableBuilder(
    column: $table.freshnessStatus,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get localDownloadPath => $composableBuilder(
    column: $table.localDownloadPath,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get sha256 => $composableBuilder(
    column: $table.sha256,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get sizeBytes => $composableBuilder(
    column: $table.sizeBytes,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get lastVerifiedAt => $composableBuilder(
    column: $table.lastVerifiedAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get downloadedAt => $composableBuilder(
    column: $table.downloadedAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$CachedSmartCacheFilesTableAnnotationComposer
    extends Composer<_$AppDatabase, $CachedSmartCacheFilesTable> {
  $$CachedSmartCacheFilesTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get ownerUid =>
      $composableBuilder(column: $table.ownerUid, builder: (column) => column);

  GeneratedColumn<String> get roomId =>
      $composableBuilder(column: $table.roomId, builder: (column) => column);

  GeneratedColumn<String> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get sourceRelativePath => $composableBuilder(
    column: $table.sourceRelativePath,
    builder: (column) => column,
  );

  GeneratedColumn<String> get payloadJson => $composableBuilder(
    column: $table.payloadJson,
    builder: (column) => column,
  );

  GeneratedColumn<String> get availabilityStatus => $composableBuilder(
    column: $table.availabilityStatus,
    builder: (column) => column,
  );

  GeneratedColumn<String> get freshnessStatus => $composableBuilder(
    column: $table.freshnessStatus,
    builder: (column) => column,
  );

  GeneratedColumn<String> get localDownloadPath => $composableBuilder(
    column: $table.localDownloadPath,
    builder: (column) => column,
  );

  GeneratedColumn<String> get sha256 =>
      $composableBuilder(column: $table.sha256, builder: (column) => column);

  GeneratedColumn<int> get sizeBytes =>
      $composableBuilder(column: $table.sizeBytes, builder: (column) => column);

  GeneratedColumn<DateTime> get lastVerifiedAt => $composableBuilder(
    column: $table.lastVerifiedAt,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get downloadedAt => $composableBuilder(
    column: $table.downloadedAt,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$CachedSmartCacheFilesTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $CachedSmartCacheFilesTable,
          CachedSmartCacheFile,
          $$CachedSmartCacheFilesTableFilterComposer,
          $$CachedSmartCacheFilesTableOrderingComposer,
          $$CachedSmartCacheFilesTableAnnotationComposer,
          $$CachedSmartCacheFilesTableCreateCompanionBuilder,
          $$CachedSmartCacheFilesTableUpdateCompanionBuilder,
          (
            CachedSmartCacheFile,
            BaseReferences<
              _$AppDatabase,
              $CachedSmartCacheFilesTable,
              CachedSmartCacheFile
            >,
          ),
          CachedSmartCacheFile,
          PrefetchHooks Function()
        > {
  $$CachedSmartCacheFilesTableTableManager(
    _$AppDatabase db,
    $CachedSmartCacheFilesTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$CachedSmartCacheFilesTableFilterComposer(
                $db: db,
                $table: table,
              ),
          createOrderingComposer: () =>
              $$CachedSmartCacheFilesTableOrderingComposer(
                $db: db,
                $table: table,
              ),
          createComputedFieldComposer: () =>
              $$CachedSmartCacheFilesTableAnnotationComposer(
                $db: db,
                $table: table,
              ),
          updateCompanionCallback:
              ({
                Value<String> ownerUid = const Value.absent(),
                Value<String> roomId = const Value.absent(),
                Value<String> id = const Value.absent(),
                Value<String> sourceRelativePath = const Value.absent(),
                Value<String> payloadJson = const Value.absent(),
                Value<String> availabilityStatus = const Value.absent(),
                Value<String> freshnessStatus = const Value.absent(),
                Value<String?> localDownloadPath = const Value.absent(),
                Value<String?> sha256 = const Value.absent(),
                Value<int?> sizeBytes = const Value.absent(),
                Value<DateTime?> lastVerifiedAt = const Value.absent(),
                Value<DateTime?> downloadedAt = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => CachedSmartCacheFilesCompanion(
                ownerUid: ownerUid,
                roomId: roomId,
                id: id,
                sourceRelativePath: sourceRelativePath,
                payloadJson: payloadJson,
                availabilityStatus: availabilityStatus,
                freshnessStatus: freshnessStatus,
                localDownloadPath: localDownloadPath,
                sha256: sha256,
                sizeBytes: sizeBytes,
                lastVerifiedAt: lastVerifiedAt,
                downloadedAt: downloadedAt,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String ownerUid,
                required String roomId,
                required String id,
                required String sourceRelativePath,
                required String payloadJson,
                required String availabilityStatus,
                required String freshnessStatus,
                Value<String?> localDownloadPath = const Value.absent(),
                Value<String?> sha256 = const Value.absent(),
                Value<int?> sizeBytes = const Value.absent(),
                Value<DateTime?> lastVerifiedAt = const Value.absent(),
                Value<DateTime?> downloadedAt = const Value.absent(),
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => CachedSmartCacheFilesCompanion.insert(
                ownerUid: ownerUid,
                roomId: roomId,
                id: id,
                sourceRelativePath: sourceRelativePath,
                payloadJson: payloadJson,
                availabilityStatus: availabilityStatus,
                freshnessStatus: freshnessStatus,
                localDownloadPath: localDownloadPath,
                sha256: sha256,
                sizeBytes: sizeBytes,
                lastVerifiedAt: lastVerifiedAt,
                downloadedAt: downloadedAt,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$CachedSmartCacheFilesTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $CachedSmartCacheFilesTable,
      CachedSmartCacheFile,
      $$CachedSmartCacheFilesTableFilterComposer,
      $$CachedSmartCacheFilesTableOrderingComposer,
      $$CachedSmartCacheFilesTableAnnotationComposer,
      $$CachedSmartCacheFilesTableCreateCompanionBuilder,
      $$CachedSmartCacheFilesTableUpdateCompanionBuilder,
      (
        CachedSmartCacheFile,
        BaseReferences<
          _$AppDatabase,
          $CachedSmartCacheFilesTable,
          CachedSmartCacheFile
        >,
      ),
      CachedSmartCacheFile,
      PrefetchHooks Function()
    >;

class $AppDatabaseManager {
  final _$AppDatabase _db;
  $AppDatabaseManager(this._db);
  $$CachedDevicesTableTableManager get cachedDevices =>
      $$CachedDevicesTableTableManager(_db, _db.cachedDevices);
  $$CachedRoomsTableTableManager get cachedRooms =>
      $$CachedRoomsTableTableManager(_db, _db.cachedRooms);
  $$CachedCommandsTableTableManager get cachedCommands =>
      $$CachedCommandsTableTableManager(_db, _db.cachedCommands);
  $$CachedProposalsTableTableManager get cachedProposals =>
      $$CachedProposalsTableTableManager(_db, _db.cachedProposals);
  $$CachedExecutionsTableTableManager get cachedExecutions =>
      $$CachedExecutionsTableTableManager(_db, _db.cachedExecutions);
  $$CachedRoomSnapshotsTableTableManager get cachedRoomSnapshots =>
      $$CachedRoomSnapshotsTableTableManager(_db, _db.cachedRoomSnapshots);
  $$MutationOutboxTableTableManager get mutationOutbox =>
      $$MutationOutboxTableTableManager(_db, _db.mutationOutbox);
  $$SyncCursorsTableTableManager get syncCursors =>
      $$SyncCursorsTableTableManager(_db, _db.syncCursors);
  $$CachedSmartCacheFilesTableTableManager get cachedSmartCacheFiles =>
      $$CachedSmartCacheFilesTableTableManager(_db, _db.cachedSmartCacheFiles);
}
