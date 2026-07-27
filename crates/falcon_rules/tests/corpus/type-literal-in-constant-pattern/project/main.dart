import 'types.dart' as types show RemoteType, RemoteAlias;
import 'types.dart' show Duplicate;
import 'other.dart' show Duplicate;

void check(Object? value) {
  if (value case types.RemoteType) { /* expect: type-literal-in-constant-pattern */
    print('type');
  }
  if (value case types.RemoteAlias) { /* expect: type-literal-in-constant-pattern */
    print('alias');
  }
  if (value case Duplicate) {
    print('ambiguous');
  }
}
