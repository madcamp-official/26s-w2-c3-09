import 'package:flutter/material.dart';
import 'package:mousekeeper_character_assets/character_assets.dart';

/// Blocks interaction while keeping the current screen visible beneath a
/// lightweight dim layer. The cheese fills from bottom to top while waiting.
class CheeseLoadingOverlay extends StatelessWidget {
  const CheeseLoadingOverlay({
    super.key,
    required this.loading,
    required this.child,
    this.message = '잠시만 기다려 주세요',
    this.progress,
    this.size = 66,
  });

  final bool loading;
  final Widget child;
  final String message;
  final double? progress;
  final double size;

  @override
  Widget build(BuildContext context) => Stack(
    fit: StackFit.expand,
    children: [
      child,
      if (loading)
        Positioned.fill(
          child: AbsorbPointer(
            key: const ValueKey('cheese-loading-blocker'),
            child: ColoredBox(
              key: const ValueKey('cheese-loading-dim'),
              color: Colors.black.withValues(alpha: 0.38),
              child: SafeArea(
                child: Center(
                  child: CheeseLoadingIndicator(
                    message: message,
                    progress: progress,
                    size: size,
                  ),
                ),
              ),
            ),
          ),
        ),
    ],
  );
}

class CheeseLoadingView extends StatelessWidget {
  const CheeseLoadingView({
    super.key,
    this.message = '잠시만 기다려 주세요',
    this.progress,
    this.size = 66,
  });

  final String message;
  final double? progress;
  final double size;

  @override
  Widget build(BuildContext context) => ColoredBox(
    key: const ValueKey('cheese-loading-dim'),
    color: Colors.black.withValues(alpha: 0.38),
    child: SafeArea(
      child: Center(
        child: CheeseLoadingIndicator(
          message: message,
          progress: progress,
          size: size,
        ),
      ),
    ),
  );
}

class CheeseLoadingIndicator extends StatefulWidget {
  const CheeseLoadingIndicator({
    super.key,
    this.message,
    this.progress,
    this.size = 66,
  });

  final String? message;
  final double? progress;
  final double size;

  @override
  State<CheeseLoadingIndicator> createState() => _CheeseLoadingIndicatorState();
}

class _CheeseLoadingIndicatorState extends State<CheeseLoadingIndicator>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late final Animation<double> _fill;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1450),
    );
    _fill = TweenSequence<double>([
      TweenSequenceItem(
        tween: Tween<double>(
          begin: 0.08,
          end: 1,
        ).chain(CurveTween(curve: Curves.easeInOutCubic)),
        weight: 84,
      ),
      TweenSequenceItem(tween: ConstantTween<double>(1), weight: 16),
    ]).animate(_controller);
    if (widget.progress == null) _controller.repeat();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.disableAnimationsOf(context)) {
      _controller.stop();
    } else if (widget.progress == null && !_controller.isAnimating) {
      _controller.repeat();
    }
  }

  @override
  void didUpdateWidget(CheeseLoadingIndicator oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.progress != null) {
      _controller.stop();
    } else if (!MediaQuery.disableAnimationsOf(context) &&
        !_controller.isAnimating) {
      _controller.repeat();
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final disableAnimations = MediaQuery.disableAnimationsOf(context);
    final suppliedProgress = widget.progress?.clamp(0.0, 1.0).toDouble();
    final message = widget.message;
    return Semantics(
      liveRegion: true,
      label: message ?? '로딩 중',
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (suppliedProgress != null || disableAnimations)
            _CheeseFillFrame(
              progress: suppliedProgress ?? 0.72,
              size: widget.size,
            )
          else
            AnimatedBuilder(
              animation: _fill,
              builder: (_, _) =>
                  _CheeseFillFrame(progress: _fill.value, size: widget.size),
            ),
          if (message != null && message.isNotEmpty) ...[
            const SizedBox(height: 10),
            Text(
              message,
              key: const ValueKey('cheese-loading-message'),
              textAlign: TextAlign.center,
              style: const TextStyle(
                color: Colors.white,
                fontSize: 13,
                fontWeight: FontWeight.w800,
                shadows: [Shadow(color: Colors.black54, blurRadius: 3)],
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _CheeseFillFrame extends StatelessWidget {
  const _CheeseFillFrame({required this.progress, required this.size});

  final double progress;
  final double size;

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: size,
    child: Stack(
      fit: StackFit.expand,
      children: [
        Opacity(
          opacity: 0.52,
          child: ColorFiltered(
            colorFilter: const ColorFilter.matrix(<double>[
              0.2126,
              0.7152,
              0.0722,
              0,
              0,
              0.2126,
              0.7152,
              0.0722,
              0,
              0,
              0.2126,
              0.7152,
              0.0722,
              0,
              0,
              0,
              0,
              0,
              1,
              0,
            ]),
            child: _cheeseImage(),
          ),
        ),
        ClipRect(clipper: _CheeseFillClipper(progress), child: _cheeseImage()),
      ],
    ),
  );

  Widget _cheeseImage() => Image.asset(
    mousekeeperCheeseLoadingAsset,
    package: mousekeeperMascotPackage,
    fit: BoxFit.contain,
    filterQuality: FilterQuality.none,
  );
}

class _CheeseFillClipper extends CustomClipper<Rect> {
  const _CheeseFillClipper(this.progress);

  final double progress;

  @override
  Rect getClip(Size size) {
    final top = size.height * (1 - progress.clamp(0.0, 1.0));
    return Rect.fromLTRB(0, top, size.width, size.height);
  }

  @override
  bool shouldReclip(_CheeseFillClipper oldClipper) =>
      oldClipper.progress != progress;
}
