class Example {
  Example();

  Example.named();

  factory Example.create() => Example();
}

class FirstConstructor {
  FirstConstructor() {
    final make = () { return Example(); };
    make();
  }
}

class Fields {
  final int value;

  const Fields(this.value);
}

class Documented {
  final int value;

  /// Creates a documented instance.
  Documented(this.value);
}

enum Action {
  run;

  /// Creates an action.
  const Action();
}
