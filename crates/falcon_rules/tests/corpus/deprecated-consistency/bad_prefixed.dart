import 'dart:core' as $core;

// Protobuf generators use prefixed dart:core metadata.
@$core.Deprecated('This message is deprecated')
class Pagination {
  factory Pagination() => Pagination._(); /* expect: deprecated-consistency */
  Pagination._(); /* expect: deprecated-consistency */
  factory Pagination.fromBuffer($core.List<$core.int> data) => Pagination._(); /* expect: deprecated-consistency */
  factory Pagination.fromJson($core.String json) => Pagination._(); /* expect: deprecated-consistency */
}

@$core.Deprecated.subclass()
class Subclassable {
  Subclassable(); /* expect: deprecated-consistency */
}

@$core.Deprecated.instantiate()
class Instantiable {
  Instantiable(); /* expect: deprecated-consistency */
}

class PrefixedField {
  PrefixedField(this.value); /* expect: deprecated-consistency */
  @$core.deprecated
  final $core.int value;
}

class PrefixedParameter {
  PrefixedParameter(@$core.Deprecated('old') this.value);
  final $core.int value; /* expect: deprecated-consistency */
}
