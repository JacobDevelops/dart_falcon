class Encodable {
  Object toJson() => {'value': true};
}
class Derived extends Encodable {}
class NotEncodable {}
