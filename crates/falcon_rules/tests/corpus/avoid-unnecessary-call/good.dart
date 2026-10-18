class Callable {
  void call() {}
}
void inspect(void Function()? nullable, void Function() action, Callable custom, dynamic unresolved) {
  nullable?.call();
  action();
  unresolved.call();
  final tearOff = action.call;
  action..call();
}

class StaticCallable {
  static void call() {}
}
class Plain {}
extension CallableExtension on Plain {
  void call() {}
}
void exclusions(Plain plain, Callable? nullableCallable) {
  StaticCallable.call();
  plain.call();
  nullableCallable?.call();
}
