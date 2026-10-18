import 'package:meta/meta.dart';

abstract class Widget {}
abstract class StatefulWidget extends Widget {}
abstract class State<T extends StatefulWidget> {
  @protected
  void initState() {}
  @protected
  void dispose() {}
  @protected
  void didChangeDependencies() {}
  @protected
  void deactivate() {}
  @protected
  void reassemble() {}
}
