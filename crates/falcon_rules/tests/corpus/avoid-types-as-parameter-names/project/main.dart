library parameter_names;

import 'barrel.dart' show Visible, Callback;
import 'types.dart' as p;
import 'types.dart' hide Hidden, Visible, Callback, PrefixedOnly;
part 'part.dart';

void imported(Visible) {} /* expect: avoid-types-as-parameter-names */
void alias(Callback) {} /* expect: avoid-types-as-parameter-names */
void core(String) {} /* expect: avoid-types-as-parameter-names */
void hidden(Hidden) {}
void prefixed(PrefixedOnly) {}
void unrelated(Unimported) {}
void typedImported(Object Visible) {}
void typedAlias(Object Callback) {}
void typedCore(Object String) {}
