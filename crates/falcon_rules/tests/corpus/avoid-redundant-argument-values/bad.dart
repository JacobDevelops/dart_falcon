void configure({bool verbose = false, int retries = 3, String mode = 'fast'}) {}

void positional(int a, [int b = 1, int c = 2]) {}

const defaultLimit = 10;
void limited({int limit = defaultLimit}) {}

class Widget {
  const Widget({this.size = 1.0, this.label = ''});
  Widget.named({int count = 0}) : size = 0, label = '';
  factory Widget.redirect({double size}) = Widget;
  final double size;
  final String label;

  static const zero = 0;
  void resize({int by = zero, bool animate = true}) {}

  void selfCall() {
    resize(animate: true); /* expect: avoid-redundant-argument-values */
  }
}

enum Level {
  low(weight: 1), /* expect: avoid-redundant-argument-values */
  high(weight: 5);

  const Level({this.weight = 1});
  final int weight;
}

class Marker {
  const Marker([this.value = 'x']);
  final String value;
}

@Marker('x') /* expect: avoid-redundant-argument-values */
void annotated() {}

void main() {
  configure(verbose: false); /* expect: avoid-redundant-argument-values */
  configure(retries: 3); /* expect: avoid-redundant-argument-values */
  configure(mode: 'fast', verbose: true); /* expect: avoid-redundant-argument-values */
  positional(0, 5, 2); /* expect: avoid-redundant-argument-values */
  positional(0, 1); /* expect: avoid-redundant-argument-values */
  limited(limit: 10); /* expect: avoid-redundant-argument-values */
  Widget(size: 1.0); /* expect: avoid-redundant-argument-values */
  Widget(size: 1); /* expect: avoid-redundant-argument-values */
  Widget.named(count: 0); /* expect: avoid-redundant-argument-values */
  Widget.redirect(size: 1.0); /* expect: avoid-redundant-argument-values */
  const Widget(label: ''); /* expect: avoid-redundant-argument-values */
  final w = Widget();
  w.resize(by: 0); /* expect: avoid-redundant-argument-values */
  w.resize(animate: !false); /* expect: avoid-redundant-argument-values */
}
