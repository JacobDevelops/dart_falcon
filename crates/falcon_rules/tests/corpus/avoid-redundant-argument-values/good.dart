void configure({bool verbose = false, int retries = 3, String? name}) {}

void positional(int a, [int b = 1, int c = 2]) {}

void required({required int count}) {}

void numeric({num value = 1}) {}

void listDefault({List<int> values = const []}) {}

class Widget {
  Widget({this.size = 1.0});
  factory Widget.redirect({double size}) = Widget.other;
  Widget.other({this.size = 2.0});
  final double size;
}

void main(bool flag, int n) {
  configure(verbose: true);
  configure(retries: 4);
  configure(name: null);
  configure(verbose: flag);
  positional(0, 1, 3);
  positional(0, 1, 2 + 1);
  required(count: 0);
  numeric(value: 1.0);
  listDefault(values: []);
  configure(retries: n);
  Widget(size: 2.0);
  Widget.redirect(size: 1.0);
  print(1);
  final local = (int x, {int y = 0}) => x + y;
  local(1, y: 0);
}

void shadowed(void Function({bool verbose}) configure) {
  configure(verbose: false);
}
