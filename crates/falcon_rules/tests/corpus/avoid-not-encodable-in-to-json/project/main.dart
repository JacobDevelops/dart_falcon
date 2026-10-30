import 'models.dart';
import 'package:encoding_dependency/models.dart';
class Serialized {
  Object toJson(NotEncodable plain, Encodable encoded, Derived child) => {
    'plain': plain, /* expect: avoid-not-encodable-in-to-json */
    'encoded': encoded,
    'child': child,
  };
}

class Point extends Equatable {}
class DependencySerialized {
  Object toJson(DepModel dependency, Point point, DepChild child,
      DepEncodable encoded) => {
    'dependency': dependency, /* expect: avoid-not-encodable-in-to-json */
    'point': point, /* expect: avoid-not-encodable-in-to-json */
    'child': child, /* expect: avoid-not-encodable-in-to-json */
    'encoded': encoded,
  };
}
