import 'package:flutter/material.dart';

abstract final class PixelColors {
  static const ink = Color(0xFF2D211C);
  static const paper = Color(0xFFFFF8E8);
  static const canvas = Color(0xFFF3DDBF);
  static const brown = Color(0xFF875F45);
  static const caramel = Color(0xFFD89A54);
  static const cream = Color(0xFFFFE7B0);
  static const sage = Color(0xFF8C9A62);
  static const red = Color(0xFFB84C3D);
  static const muted = Color(0xFF746358);
}

abstract final class PixelTheme {
  static ThemeData get light {
    const square = RoundedRectangleBorder(
      borderRadius: BorderRadius.zero,
      side: BorderSide(color: PixelColors.ink, width: 2),
    );
    final scheme = ColorScheme.fromSeed(
      seedColor: PixelColors.brown,
      brightness: Brightness.light,
      surface: PixelColors.paper,
      error: PixelColors.red,
    );
    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      scaffoldBackgroundColor: PixelColors.canvas,
      fontFamily: 'Galmuri11',
      textTheme: const TextTheme(
        displaySmall: TextStyle(fontWeight: FontWeight.w900, letterSpacing: 1),
        headlineMedium: TextStyle(
          fontWeight: FontWeight.w900,
          letterSpacing: .5,
        ),
        titleLarge: TextStyle(fontWeight: FontWeight.w900),
        titleMedium: TextStyle(fontWeight: FontWeight.w800),
        bodyLarge: TextStyle(fontWeight: FontWeight.w600, height: 1.4),
        bodyMedium: TextStyle(fontWeight: FontWeight.w600, height: 1.4),
        labelLarge: TextStyle(fontWeight: FontWeight.w900, letterSpacing: .5),
      ).apply(bodyColor: PixelColors.ink, displayColor: PixelColors.ink),
      appBarTheme: const AppBarTheme(
        backgroundColor: PixelColors.canvas,
        foregroundColor: PixelColors.ink,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        titleTextStyle: TextStyle(
          color: PixelColors.ink,
          fontFamily: 'Galmuri11',
          fontWeight: FontWeight.w900,
          fontSize: 20,
          letterSpacing: 1,
        ),
      ),
      cardTheme: const CardThemeData(
        color: PixelColors.paper,
        surfaceTintColor: Colors.transparent,
        shape: square,
        elevation: 0,
      ),
      dialogTheme: const DialogThemeData(
        backgroundColor: PixelColors.paper,
        shape: square,
      ),
      inputDecorationTheme: const InputDecorationTheme(
        filled: true,
        fillColor: PixelColors.paper,
        border: OutlineInputBorder(borderRadius: BorderRadius.zero),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.zero,
          borderSide: BorderSide(color: PixelColors.ink, width: 2),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.zero,
          borderSide: BorderSide(color: PixelColors.brown, width: 3),
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          backgroundColor: PixelColors.brown,
          foregroundColor: PixelColors.paper,
          disabledBackgroundColor: PixelColors.muted,
          minimumSize: const Size(48, 52),
          shape: square,
          textStyle: const TextStyle(
            fontFamily: 'Galmuri11',
            fontWeight: FontWeight.w900,
          ),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          foregroundColor: PixelColors.ink,
          minimumSize: const Size(48, 52),
          side: const BorderSide(color: PixelColors.ink, width: 2),
          shape: square,
          textStyle: const TextStyle(
            fontFamily: 'Galmuri11',
            fontWeight: FontWeight.w900,
          ),
        ),
      ),
      progressIndicatorTheme: const ProgressIndicatorThemeData(
        color: PixelColors.brown,
        linearTrackColor: PixelColors.cream,
      ),
      snackBarTheme: const SnackBarThemeData(
        backgroundColor: PixelColors.ink,
        contentTextStyle: TextStyle(
          color: PixelColors.paper,
          fontFamily: 'Galmuri11',
        ),
        shape: square,
        behavior: SnackBarBehavior.floating,
      ),
    );
  }
}

class PixelPanel extends StatelessWidget {
  const PixelPanel({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(20),
    this.color = PixelColors.paper,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;
  final Color color;

  @override
  Widget build(BuildContext context) => Container(
    decoration: BoxDecoration(
      color: color,
      border: Border.all(color: PixelColors.ink, width: 2),
      boxShadow: const [
        BoxShadow(color: PixelColors.ink, offset: Offset(5, 5)),
      ],
    ),
    padding: padding,
    child: child,
  );
}

class PixelLabel extends StatelessWidget {
  const PixelLabel(this.text, {super.key, this.color = PixelColors.caramel});

  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: BoxDecoration(
      color: color,
      border: Border.all(color: PixelColors.ink, width: 2),
    ),
    child: Padding(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      child: Text(text, style: Theme.of(context).textTheme.labelLarge),
    ),
  );
}
