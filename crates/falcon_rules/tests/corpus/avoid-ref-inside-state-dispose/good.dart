import 'package:flutter_riverpod/flutter_riverpod.dart';
abstract class Good extends ConsumerState<Object> {
  void initState() { ref.read(Object()); }
  void build() { this.ref.read(Object()); }
  void dispose() {
    final ref = FakeRef();
    ref.read(Object());
    super.dispose();
  }
  static void disposeOther(WidgetRef ref) { ref.read(Object()); }
}
class FakeRef { void read(Object value) {} }
class Other { WidgetRef? ref; void dispose() { ref?.read(Object()); } }
void dispose(WidgetRef ref) { ref.read(Object()); }

mixin BuildMixin<W extends Object> on ConsumerState<W> {
  void build() { ref.read(Object()); }
  @override
  void dispose() {
    final ref = FakeRef();
    ref.read(Object());
    super.dispose();
  }
}
mixin OtherMixin {
  WidgetRef get ref;
  void dispose() { ref.read(Object()); }
}
