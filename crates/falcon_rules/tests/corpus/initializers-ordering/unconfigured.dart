// No explicit order option: declaration order is active.
class Example {
  final int zebra;
  final int alpha;
  Example() : alpha = 1, /* expect: initializers-ordering */ zebra = 2;
}
