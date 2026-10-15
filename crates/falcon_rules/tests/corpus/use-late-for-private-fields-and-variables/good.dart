late int _lateInt;

int? publicNullable;

int? _readPlain;

int? _assignedNull;

int _nonNullable = 0;

void use() {
  _lateInt = 1;
  print(_lateInt);
  publicNullable = 1;
  print(publicNullable!);
  print(_readPlain);
  _assignedNull = null;
  print(_assignedNull!);
  _nonNullable = 2;
  print(_nonNullable);
}

int? _compound;

void bump() {
  _compound ??= 1;
  print(_compound!);
}

class Holder {
  int? _initialized;
  int? _publicRead;
  int? _nullableWrite;

  Holder(this._initialized);

  int get value => _initialized!;
  int? get raw => _publicRead;

  void write(int? next) {
    _nullableWrite = next;
    print(_nullableWrite!);
  }
}

class ConstHolder {
  final int? _value;
  const ConstHolder(this._value);
  int get value => _value!;
}

enum Mode {
  a;

  static int? _cached;
  static int get cached => _cached!;
}

class Initialized {
  String? _name;
  Initialized() : _name = 'x';
  String get name => _name!;
}

// Through `this.` or another receiver, the analyzer also visits the bare
// property name, which counts as a nullable read even under `!`.
class ThisAsserted {
  int? _count;

  void reset() {
    this._count = 0;
  }

  int next() => this._count! + 1;
}

class PlainReader {
  int? _count;

  void reset() {
    _count = 0;
  }

  int? peek(PlainReader other) => other._count;
}

class Shadowed {
  int? _total;

  void reset() {
    _total = 0;
  }

  int? read() => _total;
}

// `Type._x!` is a prefixed identifier: its bare name is a nullable read.
class QualifiedRead {
  static int? _shared;

  static void init() {
    _shared = 1;
  }
}

int sharedValue() => QualifiedRead._shared! + 1;
