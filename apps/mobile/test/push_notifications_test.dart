import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/notifications/push_notifications.dart';

void main() {
  test('validated FCM data becomes a foreground notice', () {
    final notice = pushNoticeForMessage(
      data: const {
        'eventType': 'proposal.created',
        'syncEventId': '018f4c7b-1ad6-7c95-bf34-5e45881f98a1',
      },
      notificationBody: '정리안을 확인해 주세요.',
    );
    expect(notice?.eventType, 'proposal.created');
    expect(notice?.message, '정리안을 확인해 주세요.');
  });

  test('malformed FCM data is ignored', () {
    expect(
      pushNoticeForMessage(
        data: const {'eventType': 'proposal.created'},
        notificationBody: 'missing event id',
      ),
      isNull,
    );
    expect(
      pushNoticeForMessage(
        data: const {
          'eventType': 'proposal.created',
          'syncEventId': 'event-id',
        },
        notificationBody: ' ',
      ),
      isNull,
    );
  });
}
