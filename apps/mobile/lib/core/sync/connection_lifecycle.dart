import 'package:flutter_riverpod/flutter_riverpod.dart';

class ConnectionLifecycleEvent {
  const ConnectionLifecycleEvent({
    required this.eventId,
    required this.eventType,
    this.deviceId,
    this.device,
    this.roomId,
    this.sequence,
  });

  final String eventId;
  final String eventType;
  final String? deviceId;
  final Map<String, dynamic>? device;
  final String? roomId;
  final int? sequence;
}

ConnectionLifecycleEvent? connectionLifecycleEventFor(
  String socketEvent,
  Object? data,
) {
  if (data is! Map) return null;
  final envelope = Map<String, dynamic>.from(data);
  final envelopeEventType = envelope['eventType'];
  final eventType = envelopeEventType is String
      ? envelopeEventType
      : socketEvent;
  if (!const {
    'device.paired',
    'device.revoked',
    'room.removed',
  }.contains(eventType)) {
    return null;
  }
  final rawPayload = envelope['payload'];
  if (rawPayload is! Map) return null;
  final payload = Map<String, dynamic>.from(rawPayload);
  final aggregateId = _requiredId(envelope['aggregateId']);
  final eventId = _requiredId(envelope['eventId']);
  final sequence = envelope['sequence'];
  if (eventId == null ||
      aggregateId == null ||
      sequence is! int ||
      sequence < 0) {
    return null;
  }

  final expectedStatus = switch (eventType) {
    'device.paired' => 'ACTIVE',
    'device.revoked' => 'REVOKED',
    'room.removed' => 'REMOVED',
    _ => null,
  };
  if (payload['status'] != expectedStatus) return null;

  final deviceId = eventType.startsWith('device.')
      ? _requiredId(payload['deviceId'])
      : null;
  Map<String, dynamic>? device;
  if (eventType == 'device.paired') {
    final rawDevice = payload['device'];
    if (rawDevice != null) {
      device = _activeDevicePayload(rawDevice, deviceId);
      if (device == null) return null;
    }
  }
  final roomId = eventType == 'room.removed'
      ? _requiredId(payload['roomId'])
      : null;
  final payloadAggregateId = deviceId ?? roomId;
  if (payloadAggregateId == null || payloadAggregateId != aggregateId) {
    return null;
  }
  return ConnectionLifecycleEvent(
    eventId: eventId,
    eventType: eventType,
    deviceId: deviceId,
    device: device,
    roomId: roomId,
    sequence: sequence,
  );
}

final RegExp _canonicalUuid = RegExp(
  r'^(?:[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}|00000000-0000-0000-0000-000000000000|ffffffff-ffff-ffff-ffff-ffffffffffff)$',
);

String? _requiredId(Object? value) =>
    value is String && _canonicalUuid.hasMatch(value) ? value : null;

Map<String, dynamic>? _activeDevicePayload(Object? value, String? deviceId) {
  if (deviceId == null || value is! Map) return null;
  final device = Map<String, dynamic>.from(value);
  if (device['id'] != deviceId || device['status'] != 'ACTIVE') {
    return null;
  }
  if (device['platform'] is! String ||
      device['deviceName'] is! String ||
      device['createdAt'] is! String ||
      !(device['lastSeenAt'] == null || device['lastSeenAt'] is String)) {
    return null;
  }
  return device;
}

final connectionLifecycleEventProvider =
    NotifierProvider<
      ConnectionLifecycleEventController,
      ConnectionLifecycleEvent?
    >(ConnectionLifecycleEventController.new);

class ConnectionLifecycleEventController
    extends Notifier<ConnectionLifecycleEvent?> {
  @override
  ConnectionLifecycleEvent? build() => null;

  void emit(ConnectionLifecycleEvent event) {
    if (state?.eventId == event.eventId) return;
    state = event;
  }
}
