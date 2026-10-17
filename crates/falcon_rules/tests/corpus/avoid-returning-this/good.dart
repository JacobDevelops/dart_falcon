class Builder {
  final List<int> _items = [];

  // Use cascades instead of returning `this`.
  void add(int x) => _items.add(x);

  // Returns a new instance, not `this`.
  Builder copy() => Builder().._items.addAll(_items);

  // Some paths return something else.
  Builder pick(Builder other, bool first) {
    if (first) return this;
    return other;
  }

  // Return type is not the enclosing type.
  Object identity() => this;

  // Operators are not methods.
  Builder operator +(int x) => this;
}

// Overriding an inherited member is required by the interface.
abstract class Chain {
  Chain next();
}

class Link implements Chain {
  @override
  Chain next() => this;
}

class Node extends Link {
  @override
  Link next() => this;
}


// Extensions are not checked.
extension on String {
  String same() => this;
}

// Static methods cannot return `this`; nested closures do not count.
class Holder {
  Holder run() {
    final f = () => this;
    return Holder();
  }
}

class GroupedReceiver {
  GroupedReceiver arrow() => (this);
  GroupedReceiver block() {
    return (/* grouping is significant to this lint */ this);
  }
  GroupedReceiver mixed(bool flag) {
    if (flag) return this;
    return ((this));
  }
}

class NestedOnly {
  NestedOnly run() {
    NestedOnly local() {
      return this;
    }
    final callback = () {
      return this;
    };
  }
}

class ReturnedClosure {
  ReturnedClosure run() {
    return (() {
      return this;
    })();
  }
}
