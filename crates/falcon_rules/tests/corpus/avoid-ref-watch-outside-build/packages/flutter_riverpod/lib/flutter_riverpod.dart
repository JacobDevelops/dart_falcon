export 'package:riverpod/riverpod.dart';
import 'package:riverpod/riverpod.dart';
abstract class WidgetRef implements Ref {}
abstract class ConsumerWidget {
  const ConsumerWidget();
  Object build(Object context, WidgetRef ref);
}
abstract class ConsumerState<T> {
  WidgetRef get ref;
  Object build(Object context);
}
class Consumer {
  Consumer({required Object Function(Object, WidgetRef, Object?) builder});
}
