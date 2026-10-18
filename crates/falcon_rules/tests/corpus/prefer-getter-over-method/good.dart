class Box {
  int _size = 3;
  late int _lazy;
  external int externalValue;
  int readExternal() => externalValue;
  final String _label = 'box';
  String toString() => _label;
  int call() => _size;
  final Map<String, Object?> _json = {};
  Map<String, Object?> toJson() => _json;
  int get size => _size;
  int compute() => _size + 1;
  int update() => ++_size;
  int lazy() => _lazy;
  int withArgument(int value) => _size;
  T generic<T>() => throw StateError('missing');
  Future<int> asyncSize() async => _size;
  static int staticSize() => 3;
  int get dynamicValue => _size;
  int fromGetter() => dynamicValue;
  int sideEffect() { _size++; return _size; }
}
class Child extends Box {
  int inherited() => _size;
}
