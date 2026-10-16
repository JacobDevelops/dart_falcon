void configure(int level, bool verbose) {} /* expect: avoid-positional-boolean-parameters */

// Only the first positional bool is reported.
void two(bool verbose, bool dryRun) {} /* expect: avoid-positional-boolean-parameters */

// Optional positional bools count too.
void optional(int level, [bool flag = false]) {} /* expect: avoid-positional-boolean-parameters */

void nullable(bool? flag) {} /* expect: avoid-positional-boolean-parameters */

// Top-level setters are functions to the analyzer.
set enabled(bool value) {} /* expect: avoid-positional-boolean-parameters */

class Widget {
  final bool enabled;
  final bool visible;

  Widget(this.enabled, this.visible); /* expect: avoid-positional-boolean-parameters */

  Widget.typed(bool enabled, bool visible) /* expect: avoid-positional-boolean-parameters */
      : enabled = enabled,
        visible = visible;

  void update(bool animate) {} /* expect: avoid-positional-boolean-parameters */

  static void create(bool a) {} /* expect: avoid-positional-boolean-parameters */
}

// The unnamed constructor of a private class is public.
class _Private {
  _Private(bool flag); /* expect: avoid-positional-boolean-parameters */
}

class Base {
  final bool flag;
  Base(this.flag); /* expect: avoid-positional-boolean-parameters */
  void listen(void Function(bool) callback) {} /* expect: avoid-positional-boolean-parameters */
}

// Positional super parameters match the super constructor by position.
class Renamed extends Base {
  Renamed(super.other); /* expect: avoid-positional-boolean-parameters */
}

class Sub extends Base {
  Sub() : super(true);

  // Function types are checked even in overriding signatures.
  @override
  void listen(void Function(bool) callback) {} /* expect: avoid-positional-boolean-parameters */
}

extension WidgetX on Widget {
  void toggle(bool a) {} /* expect: avoid-positional-boolean-parameters */
}

typedef Toggle = void Function(int, bool); /* expect: avoid-positional-boolean-parameters */

void f() {
  void local(bool a) {} /* expect: avoid-positional-boolean-parameters */
  local(true);
  _Private(true);
}

void annotated(
  @Deprecated('use named') /* expect: avoid-positional-boolean-parameters */
  bool flag,
) {}

class Updatable {
  void update(bool animate) {} /* expect: avoid-positional-boolean-parameters */
}

class Overriding extends Updatable {
  // Overrides keep the inherited signature.
  @override
  void update(bool animate) {}
}

class Unannotated extends Updatable {
  // An override without the annotation is still detected.
  void update(bool animate) {}
}

class PrimaryBoolean(bool enabled); /* expect: avoid-positional-boolean-parameters */

class DeclaringPrimaryBoolean(final bool enabled); /* expect: avoid-positional-boolean-parameters */

// Extension-type representation parameters are primary constructor parameters.
extension type BooleanRepresentation(bool enabled) {} /* expect: avoid-positional-boolean-parameters */
