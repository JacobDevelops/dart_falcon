class Parent {}
class Child extends Parent {}
class Grandchild extends Child {}
class Example {
  Object toJson() => {
    'child': Child(), /* expect: avoid-not-encodable-in-to-json */
    'grandchild': Grandchild(), /* expect: avoid-not-encodable-in-to-json */
  };
}
