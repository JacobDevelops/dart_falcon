import 'package:riverpod/riverpod.dart';
class Mutable { int value = 0; }
final family = ProviderFamily<int, Mutable>();
final future = FutureProviderFamily<int, Mutable>();
final automatic = AutoDisposeProviderFamily<int, Mutable>();
void bad(Mutable mutable) {
  family(mutable);
  family(Mutable()); /* expect: prefer-immutable-provider-arguments */
  future(Mutable()); /* expect: prefer-immutable-provider-arguments */
  automatic(Mutable()); /* expect: prefer-immutable-provider-arguments */
  final alias = family;
  alias(Mutable()); /* expect: prefer-immutable-provider-arguments */
}
