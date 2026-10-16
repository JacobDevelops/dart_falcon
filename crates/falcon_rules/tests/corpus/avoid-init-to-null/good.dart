int nonNull = 0;
late int? lazy;

class C {
  int? uninitialized;
  final String? name = null;
  const C();
  int count = 0;

  void method(int? param) {
    int? local;
    var x = 42;
    String value = 'hi';
    print('$local $x $value $param');
  }

  void withDefault({int? param = 1}) {
    print(param);
  }
}

class NonNullSuperDefault {
  NonNullSuperDefault({int? value = 3});
}

class ChangedSuperDefault extends NonNullSuperDefault {
  ChangedSuperDefault({super.value = null});
}
