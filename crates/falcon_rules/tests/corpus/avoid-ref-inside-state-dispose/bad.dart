import 'package:flutter_riverpod/flutter_riverpod.dart';
abstract class First extends ConsumerState<Object> {
  void dispose() {
    ref.read(Object()); /* expect: avoid-ref-inside-state-dispose */
    this.ref.read(Object()); /* expect: avoid-ref-inside-state-dispose */
    final cached = ref; /* expect: avoid-ref-inside-state-dispose */
    use(ref); /* expect: avoid-ref-inside-state-dispose */
    (() { ref.read(Object()); })(); /* expect: avoid-ref-inside-state-dispose */
    super.dispose();
  }
}
void use(WidgetRef value) {}
