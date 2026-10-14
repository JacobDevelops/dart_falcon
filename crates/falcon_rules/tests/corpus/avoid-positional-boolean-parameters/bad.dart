void configure(bool verbose, bool dryRun) {} /* expect: avoid-positional-boolean-parameters */

void optional(int level, [bool a = false, bool? b]) {} /* expect: avoid-positional-boolean-parameters */

class Widget {
  final bool enabled;
  final bool visible;

  Widget(this.enabled, this.visible); /* expect: avoid-positional-boolean-parameters */

  Widget.typed(bool enabled, bool visible) /* expect: avoid-positional-boolean-parameters */
      : enabled = enabled,
        visible = visible;

  void update(bool animate, bool force, bool notify) {} /* expect: avoid-positional-boolean-parameters */

  static void create(bool a, bool b) {} /* expect: avoid-positional-boolean-parameters */
}

extension WidgetX on Widget {
  void toggle(bool a, bool b) {} /* expect: avoid-positional-boolean-parameters */
}

typedef Toggle = void Function(bool, bool); /* expect: avoid-positional-boolean-parameters */

void f() {
  void local(bool a, bool b) {} /* expect: avoid-positional-boolean-parameters */
  local(true, false);
}

class Base {
  void update(bool animate, bool force) {} /* expect: avoid-positional-boolean-parameters */
  void listen(void Function(bool, bool) callback) {} /* expect: avoid-positional-boolean-parameters */
}

class Sub extends Base {
  // Overrides keep the inherited signature.
  @override
  void update(bool animate, bool force) {}
}

class Sub2 extends Base {
  // An override without the annotation is still detected.
  void update(bool animate, bool force) {}

  // Function types in an overriding signature are fixed too.
  @override
  void listen(void Function(bool, bool) callback) {}
}
