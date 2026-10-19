class Widget {}
class StatefulWidget extends Widget {}
class BuildContext { bool get mounted => true; }
abstract class State<T extends StatefulWidget> {
  bool get mounted => true;
  BuildContext get context => BuildContext();
  void setState(void Function() callback) { callback(); }
  void dispose() {}
}
