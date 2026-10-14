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
