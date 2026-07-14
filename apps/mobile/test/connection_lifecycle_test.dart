import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/sync/connection_lifecycle.dart';

void main() {
  test('live revoke envelope uses its canonical aggregate id', () {
    final event = connectionLifecycleEventFor('device.revoked', {
      'eventId': _eventId,
      'eventType': 'device.revoked',
      'aggregateId': _deviceA,
      'sequence': 12,
      'payload': {'deviceId': _deviceA, 'status': 'REVOKED'},
    });

    expect(event, isNotNull);
    expect(event!.deviceId, _deviceA);
    expect(event.sequence, 12);
  });

  test('replay room removal reads eventType from replay envelope', () {
    final event = connectionLifecycleEventFor('ignored-socket-name', {
      'eventId': _eventId,
      'eventType': 'room.removed',
      'aggregateId': _roomA,
      'sequence': 13,
      'payload': {'roomId': _roomA, 'status': 'REMOVED'},
    });

    expect(event?.roomId, _roomA);
    expect(event?.eventType, 'room.removed');
  });

  test('paired device envelope can carry the active device projection', () {
    final event = connectionLifecycleEventFor('device.paired', {
      'eventId': _eventId,
      'eventType': 'device.paired',
      'aggregateId': _deviceA,
      'sequence': 14,
      'payload': {
        'deviceId': _deviceA,
        'status': 'ACTIVE',
        'device': {
          'id': _deviceA,
          'platform': 'WINDOWS',
          'deviceName': 'Desktop',
          'status': 'ACTIVE',
          'lastSeenAt': null,
          'createdAt': '2026-07-13T01:02:03.000Z',
        },
      },
    });

    expect(event?.deviceId, _deviceA);
    expect(event?.device?['deviceName'], 'Desktop');
    expect(event?.sequence, 14);
  });

  test('unrelated realtime events do not enter the connection reducer', () {
    expect(
      connectionLifecycleEventFor('execution.updated', {
        'eventId': 'event-3',
        'payload': {'status': 'SUCCEEDED'},
      }),
      isNull,
    );
  });

  test('each lifecycle event requires its exact terminal status', () {
    for (final invalid in <Map<String, dynamic>>[
      _envelope(
        eventType: 'device.paired',
        aggregateId: _deviceA,
        payload: {'deviceId': _deviceA, 'status': 'REVOKED'},
      ),
      _envelope(
        eventType: 'device.paired',
        aggregateId: _deviceA,
        payload: {
          'deviceId': _deviceA,
          'status': 'ACTIVE',
          'device': {
            'id': _deviceB,
            'platform': 'WINDOWS',
            'deviceName': 'Wrong desktop',
            'status': 'ACTIVE',
            'lastSeenAt': null,
            'createdAt': '2026-07-13T01:02:03.000Z',
          },
        },
      ),
      _envelope(
        eventType: 'device.revoked',
        aggregateId: _deviceA,
        payload: {'deviceId': _deviceA, 'status': 'ACTIVE'},
      ),
      _envelope(
        eventType: 'room.removed',
        aggregateId: _roomA,
        payload: {'roomId': _roomA, 'status': 'ACTIVE'},
      ),
    ]) {
      expect(
        connectionLifecycleEventFor(invalid['eventType'] as String, invalid),
        isNull,
      );
    }
  });

  test(
    'aggregate and event-specific payload ids are required and must match',
    () {
      final valid = _envelope(
        eventType: 'device.revoked',
        aggregateId: _deviceA,
        payload: {'deviceId': _deviceA, 'status': 'REVOKED'},
      );

      expect(
        connectionLifecycleEventFor(
          'device.revoked',
          Map<String, dynamic>.from(valid)..remove('aggregateId'),
        ),
        isNull,
      );
      expect(
        connectionLifecycleEventFor('device.revoked', {
          ...valid,
          'payload': {'status': 'REVOKED'},
        }),
        isNull,
      );
      expect(
        connectionLifecycleEventFor('device.revoked', {
          ...valid,
          'payload': {'deviceId': _deviceB, 'status': 'REVOKED'},
        }),
        isNull,
      );
      expect(
        connectionLifecycleEventFor('device.revoked', {
          ...valid,
          'eventId': 'not-a-uuid',
        }),
        isNull,
      );
      expect(
        connectionLifecycleEventFor('device.revoked', {
          ...valid,
          'aggregateId': 'device-a',
        }),
        isNull,
      );
      expect(
        connectionLifecycleEventFor('device.revoked', {
          ...valid,
          'payload': {'deviceId': 'device-a', 'status': 'REVOKED'},
        }),
        isNull,
      );
    },
  );

  test('sequence is required to be a nonnegative integer', () {
    final valid = _envelope(
      eventType: 'room.removed',
      aggregateId: _roomA,
      payload: {'roomId': _roomA, 'status': 'REMOVED'},
    );

    for (final sequence in <Object?>[null, -1, 1.5, '1']) {
      final invalid = <String, dynamic>{...valid, 'sequence': sequence};
      expect(connectionLifecycleEventFor('room.removed', invalid), isNull);
    }
  });
}

Map<String, dynamic> _envelope({
  required String eventType,
  required String aggregateId,
  required Map<String, dynamic> payload,
}) => <String, dynamic>{
  'eventId': _eventId,
  'eventType': eventType,
  'aggregateId': aggregateId,
  'sequence': 0,
  'payload': payload,
};

const _eventId = '018f4c7b-1ad6-7c95-bf34-5e45881f98a1';
const _deviceA = '018f4c7b-1ad6-7c95-bf34-5e45881f98a2';
const _deviceB = '018f4c7b-1ad6-7c95-bf34-5e45881f98a3';
const _roomA = '018f4c7b-1ad6-7c95-bf34-5e45881f98a4';
