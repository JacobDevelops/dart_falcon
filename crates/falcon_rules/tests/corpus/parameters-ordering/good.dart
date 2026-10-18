void named({int alpha = 0, int zebra = 0}) {}
void positional(int zebra, int alpha) {}
void optional([int zebra = 0, int alpha = 0]) {}
void mixed(int zebra, int alpha, {int beta = 0, int gamma = 0}) {}
void single({int zebra = 0}) {}

class Example {
  final int alpha;
  final int zebra;
  Example({this.alpha = 0, this.zebra = 0});
}

typedef Callback = void Function({int alpha, int zebra});
