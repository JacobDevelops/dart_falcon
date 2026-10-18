typedef Callback = void Function();
class Callbacks {
  final void Function() onTap;
  void Function(int)? _onChanged;
  Callback? onAlias;
  Function? onUntypedFunction;
  static void Function()? staticCallback;
  final inferred = () {};
  int count = 0;
  Callbacks(this.onTap);
}
abstract class Contract {
  void Function() get tap;
}
class Implementation implements Contract {
  final void Function() tap;
  Implementation(this.tap);
}
