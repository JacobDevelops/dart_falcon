class Example {
  final int alpha;
  final int zebra;
  Example.descending() : zebra = 2, alpha = 1;
  Example.ascending() : alpha = 1, /* expect: initializers-ordering */ zebra = 2;
}
