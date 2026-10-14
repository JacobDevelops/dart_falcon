// A project-local `Container` is not Flutter's widget.
class Container {
  Container({required this.color, this.child});

  final int color;
  final Object? child;
}

final box = Container(color: 1, child: Object());
