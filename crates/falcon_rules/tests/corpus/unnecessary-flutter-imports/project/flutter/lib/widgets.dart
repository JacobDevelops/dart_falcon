class Widget {
  const Widget();
  const Widget.named();
  static const marker = 1;
}
class BuildContext {}
abstract class StatelessWidget extends Widget {
  const StatelessWidget();
  Widget build(BuildContext context);
}
class Placeholder extends Widget {
  const Placeholder();
}

extension WidgetExtension on Widget {
  String get label => 'widget';
  int operator +(int other) => other;
  Widget operator [](int index) => this;
  Widget operator -(Widget other) => this;
  String call() => label;
  set label(String value) {}
  int get count => 0;
  set count(int value) {}
  void operator []=(int index, Widget value) {}
}

enum WidgetKind { first }

class StatefulWidget extends Widget {}
abstract class State<T extends StatefulWidget> {
  late T widget;
  late BuildContext context;
  void setState(void Function() callback) => callback();
}

class FrameworkBox<T> {
  final T field;
  const FrameworkBox(this.field);
  const FrameworkBox.named(this.field);
}

extension FrameworkBoxExtension<T> on FrameworkBox<T> {
  T get value => field;
}

const frameworkMarker = 1;
