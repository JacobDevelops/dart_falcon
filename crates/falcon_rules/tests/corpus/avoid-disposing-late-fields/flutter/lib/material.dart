class Widget {}
class StatefulWidget extends Widget {}
class BuildContext { bool get mounted => true; }
abstract class State<T extends StatefulWidget> {
  bool get mounted => true;
  BuildContext get context => BuildContext();
  void setState(void Function() callback) { callback(); }
  void initState() {}
  void dispose() {}
}
class TextEditingController { void dispose() {} }
class AnimationController {
  AnimationController({Object? vsync});
  void forward() {}
  void repeat() {}
  double get value => 0;
  void dispose() {}
}
