class _Private {}

_Private topLevel(_Private value) => value; /* expect: library-private-types-in-public-api */ /* expect: library-private-types-in-public-api */

_Private publicVariable = _Private(); /* expect: library-private-types-in-public-api */

typedef PublicAlias = ({_Private value, void Function(_Private) callback}); /* expect: library-private-types-in-public-api */ /* expect: library-private-types-in-public-api */

class PublicClass<T extends _Private> { /* expect: library-private-types-in-public-api */
  _Private field; /* expect: library-private-types-in-public-api */
  PublicClass(this.field);
  _Private method(_Private value) => value; /* expect: library-private-types-in-public-api */ /* expect: library-private-types-in-public-api */
  _Private get getter => field; /* expect: library-private-types-in-public-api */
  set setter(_Private value) => field = value; /* expect: library-private-types-in-public-api */
}

mixin PublicMixin on _Private { /* expect: library-private-types-in-public-api */
  _Private operator +(covariant _Private other) => other; /* expect: library-private-types-in-public-api */ /* expect: library-private-types-in-public-api */
}

extension PublicExtension on _Private {} /* expect: library-private-types-in-public-api */

extension type PublicExtensionType(_Private value) {} /* expect: library-private-types-in-public-api */
