class Base {
  Base({void Function(int)? callback});
}
class WiderCallback extends Base {
  WiderCallback({void Function(num)? callback}) : super(callback: callback);
}
class NamedBase {
  NamedBase({void Function({required int x, required int y})? callback});
}
class OptionalNamed extends NamedBase {
  OptionalNamed({void Function({required int x, int y})? callback})
    : super(callback: callback);
}
