class A {
  int _x = 0;
  int _count = 0;

  int get x => _x;

  int get y {
    return _x + 1;
  }

  int get count => _count;

  int get doubled => _x * 2;

  String get label => 'label';

  num get w => this._x;
}

int _g = 0;

int get topLevel => _g;
