import 'package:riverpod/riverpod.dart' as rp;
import 'package:meta/meta.dart';
@immutable
class Key { final int value; const Key(this.value); }
final integer = rp.ProviderFamily<int, int>();
final string = rp.ProviderFamily<int, String>();
final boolean = rp.ProviderFamily<int, bool>();
final key = rp.ProviderFamily<int, Key>();
final unknown = rp.ProviderFamily<int, dynamic>();
class ProviderFamily { void call(Object argument) {} }
class Mutable { int value = 0; }
void good(dynamic value) {
  integer(1);
  string('key');
  boolean(true);
  key(const Key(1));
  unknown(value);
  ProviderFamily()(Mutable());
}
