class Example {
  final int zebra;
  final int alpha;
  Example() : alpha = 1, /* expect: initializers-ordering */ zebra = 2;
  Example.values(int a, int z)
      : alpha = a, /* expect: initializers-ordering */ zebra = z;
}

class Words {
  final String zebra, alpha;
  Words() : alpha = 'a', /* expect: initializers-ordering */ zebra = 'z';
}

class WithBoundary {
  final int zebra;
  final int alpha;
  WithBoundary()
      : assert(true),
        alpha = 1,
        /* expect: initializers-ordering */ zebra = 2;
}

enum EnumFields {
  value;

  final int zebra;
  final int alpha;
  const EnumFields()
      : alpha = 1, /* expect: initializers-ordering */ zebra = 2;
}
