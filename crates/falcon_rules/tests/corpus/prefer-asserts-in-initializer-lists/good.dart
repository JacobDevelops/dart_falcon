class Range {
  final int start;
  final int end;
  int get length => end - start;
  Range(this.start, this.end) : assert(start <= end);
  Range.checked(this.start, this.end) {
    assert(length >= 0);
  }
  Range.self(this.start, this.end) {
    assert(this.start <= this.end);
  }
  Range.method(this.start, this.end) {
    assert(validate());
  }
  Range.late(int a, int b) : start = a, end = b {
    print(a);
    assert(a <= b);
  }
  factory Range.make(int a) {
    assert(a > 0);
    return Range(a, a);
  }
  Range.other(Range other) : start = other.start, end = other.end {
    assert(other.length >= 0);
  }
  bool validate() => start <= end;
}

class External extends Object with Mixed {
  final int x;
  External(this.x) {
    assert(mixed > 0);
  }
}

mixin Mixed {
  int get mixed => 1;
}
