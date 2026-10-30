class Widget {}
class StatefulWidget extends Widget {}

abstract class State<T extends StatefulWidget> {
  void initState() {}
  void didUpdateWidget(T oldWidget) {}
  void activate() {}
  void didChangeDependencies() {}
  void deactivate() {}
  void dispose() {}
}
