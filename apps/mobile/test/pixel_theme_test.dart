import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mousekeeper/core/theme/pixel_theme.dart';

void main() {
  test('headings are bold while body copy stays normal Galmuri', () {
    final theme = PixelTheme.light;

    expect(theme.textTheme.titleLarge?.fontFamily, mouseKeeperFontFamily);
    expect(theme.textTheme.titleLarge?.fontWeight, FontWeight.w700);
    expect(theme.textTheme.bodyMedium?.fontFamily, mouseKeeperFontFamily);
    expect(theme.textTheme.bodyMedium?.fontWeight, FontWeight.w400);

    final buttonTextStyle = theme.outlinedButtonTheme.style?.textStyle?.resolve(
      const <WidgetState>{},
    );
    expect(buttonTextStyle?.fontFamily, mouseKeeperFontFamily);
    expect(buttonTextStyle?.fontWeight, FontWeight.w700);
  });
}
