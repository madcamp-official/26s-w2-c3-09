import 'dart:async';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../network/api_client.dart';
import '../sync/realtime_controller.dart';

enum PushNotificationStatus { active, permissionDenied, unconfigured }

class PushNotificationRegistration {
  const PushNotificationRegistration({
    required this.status,
    this.registrationId,
    this.errorCode,
  });

  final PushNotificationStatus status;
  final String? registrationId;
  final String? errorCode;
}

final pushNotificationsProvider =
    AsyncNotifierProvider<
      PushNotificationsController,
      PushNotificationRegistration
    >(PushNotificationsController.new);

RealtimeNotice? pushNoticeForMessage({
  required Map<String, dynamic> data,
  String? notificationBody,
}) {
  final eventType = data['eventType'];
  final syncEventId = data['syncEventId'];
  if (eventType is! String || syncEventId is! String) return null;
  final message = notificationBody?.trim();
  if (message == null || message.isEmpty) return null;
  return RealtimeNotice(
    eventId: syncEventId,
    eventType: eventType,
    message: message,
  );
}

class PushNotificationsController
    extends AsyncNotifier<PushNotificationRegistration> {
  StreamSubscription<String>? _tokenRefreshSubscription;
  StreamSubscription<RemoteMessage>? _foregroundSubscription;
  StreamSubscription<RemoteMessage>? _openedAppSubscription;
  String? _registrationId;

  @override
  Future<PushNotificationRegistration> build() async {
    ref.onDispose(() {
      unawaited(_tokenRefreshSubscription?.cancel());
      unawaited(_foregroundSubscription?.cancel());
      unawaited(_openedAppSubscription?.cancel());
    });

    final messaging = FirebaseMessaging.instance;
    final settings = await messaging.requestPermission(
      alert: true,
      badge: true,
      sound: true,
    );
    if (settings.authorizationStatus == AuthorizationStatus.denied) {
      return const PushNotificationRegistration(
        status: PushNotificationStatus.permissionDenied,
        errorCode: 'PERMISSION_DENIED',
      );
    }
    final token = await messaging.getToken();
    if (token == null || token.isEmpty) {
      return const PushNotificationRegistration(
        status: PushNotificationStatus.unconfigured,
        errorCode: 'UNCONFIGURED',
      );
    }

    final registration = await _register(token);
    _registrationId = registration.registrationId;
    _tokenRefreshSubscription = messaging.onTokenRefresh.listen((nextToken) {
      unawaited(_refreshToken(nextToken));
    });
    _foregroundSubscription = FirebaseMessaging.onMessage.listen(_emitNotice);
    _openedAppSubscription = FirebaseMessaging.onMessageOpenedApp.listen(
      _emitNotice,
    );
    final initialMessage = await messaging.getInitialMessage();
    if (initialMessage != null) _emitNotice(initialMessage);
    return registration;
  }

  Future<PushNotificationRegistration> _register(String token) async {
    final response = await ref.read(apiClientProvider).post(
      '/v1/notification-tokens',
      {'token': token, 'platform': 'ANDROID'},
    );
    final registrationId = response['id'];
    if (registrationId is! String) {
      throw StateError('INVALID_NOTIFICATION_TOKEN_RESPONSE');
    }
    return PushNotificationRegistration(
      status: PushNotificationStatus.active,
      registrationId: registrationId,
    );
  }

  Future<void> _refreshToken(String token) async {
    final previousId = state.value?.registrationId;
    try {
      final next = await _register(token);
      _registrationId = next.registrationId;
      state = AsyncData(next);
      if (previousId != null && previousId != next.registrationId) {
        await ref
            .read(apiClientProvider)
            .delete('/v1/notification-tokens/$previousId');
      }
    } catch (error, stackTrace) {
      state = AsyncError(error, stackTrace);
    }
  }

  void _emitNotice(RemoteMessage message) {
    final notice = pushNoticeForMessage(
      data: message.data,
      notificationBody: message.notification?.body,
    );
    if (notice != null) {
      ref.read(realtimeNoticeProvider.notifier).emit(notice);
    }
  }

  Future<void> unregister() async {
    final registrationId = _registrationId;
    if (registrationId == null) return;
    await ref
        .read(apiClientProvider)
        .delete('/v1/notification-tokens/$registrationId');
    await FirebaseMessaging.instance.deleteToken();
    _registrationId = null;
    state = const AsyncData(
      PushNotificationRegistration(
        status: PushNotificationStatus.unconfigured,
        errorCode: 'UNREGISTERED',
      ),
    );
  }
}
