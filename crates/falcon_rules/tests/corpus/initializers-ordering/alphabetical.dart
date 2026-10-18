int next() => 1;

// Explicit alphabetical order overrides the nonalphabetical declarations.
class Example {
  final int zebra;
  final int alpha;
  Example.sorted() : alpha = 1, zebra = 2;
  Example.declarationOrder()
      : zebra = 2, /* expect: initializers-ordering */ alpha = 1;
  Example.values(int a, int z)
      : zebra = z, /* expect: initializers-ordering */ alpha = a;
  Example.assertBoundary() : zebra = 2, assert(true), alpha = 1;
  Example.effectBoundary() : zebra = next(), alpha = 1;
  Example.effects() : zebra = next(), alpha = next();
}

class Words {
  final String zebra;
  final String alpha;
  Words.sorted() : alpha = 'a', zebra = 'z';
  Words.unsorted()
      : zebra = 'z', /* expect: initializers-ordering */ alpha = 'a';
  Words.interpolated() : zebra = '${next()}', alpha = 'a';
}
