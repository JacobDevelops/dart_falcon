// A project-local `Container` is not Flutter's widget.
class Container {
  Container({this.decoration, this.child});

  final Object? decoration;
  final Object? child;
}

final box = Container(decoration: Object(), child: Object());
