import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/features/home/home_page.dart';

void main() {
  test('authoritative gate list excludes stale enriched cache rows', () {
    final merged = mergeAuthoritativeConnectionItems(
      authoritative: const [
        {'id': 'active', 'name': 'authoritative'},
      ],
      enriched: const [
        {'id': 'active', 'presence': 'ONLINE_IDLE'},
        {'id': 'removed', 'presence': 'ONLINE_IDLE'},
      ],
    );

    expect(merged.map((item) => item['id']), ['active']);
    expect(merged.single['presence'], 'ONLINE_IDLE');
    expect(merged.single['name'], 'authoritative');
  });

  test('empty authoritative list never falls back to stale entries', () {
    expect(
      mergeAuthoritativeConnectionItems(
        authoritative: const [],
        enriched: const [
          {'id': 'stale'},
        ],
      ),
      isEmpty,
    );
  });
}
