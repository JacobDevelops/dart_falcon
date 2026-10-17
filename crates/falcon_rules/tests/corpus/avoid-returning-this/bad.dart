class Builder {
  final List<int> _items = [];

  Builder add(int x) {
    _items.add(x);
    return this; /* expect: avoid-returning-this */
  }

  Builder clear() => this; /* expect: avoid-returning-this */

  Builder addAll(List<int> xs) {
    if (xs.isEmpty) {
      return this; /* expect: avoid-returning-this */
    }
    _items.addAll(xs);
    return this;
  }

  Builder get self => this; /* expect: avoid-returning-this */
}

class Generic<T> {
  Generic<T> touch() => this; /* expect: avoid-returning-this */
}

mixin Fluent {
  Fluent reset() {
    return this; /* expect: avoid-returning-this */
  }
}

enum Level {
  low;

  Level same() => this; /* expect: avoid-returning-this */
}

// An unresolved superclass does not establish an override.
class Remote extends SomeUnknownBase {
  Remote ping() => this; /* expect: avoid-returning-this */
}

class NestedReturns {
  NestedReturns run() {
    NestedReturns local() {
      return NestedReturns();
    }
    final callback = () {
      return NestedReturns();
    };
    return this; /* expect: avoid-returning-this */
  }
}

class ControlReturns {
  ControlReturns guarded(bool fail) {
    try {
      if (fail) throw StateError('failed');
      return this; /* expect: avoid-returning-this */
    } catch (_) {
      return this;
    } finally {
      return this;
    }
  }

  ControlReturns branches(int value) {
    switch (value) {
      case 0:
        return this; /* expect: avoid-returning-this */
      default:
        outer:
        for (var i = 0; i < value; i++) {
          if (i == 1) return this;
          break outer;
        }
    }
    while (value-- > 0) {
      return this;
    }
    do {
      return this;
    } while (false);
  }

  ControlReturns callbacks() {
    final callback = () {
      return this;
    };
    ControlReturns local() {
      return this;
    }
    return this; /* expect: avoid-returning-this */
  }
}
