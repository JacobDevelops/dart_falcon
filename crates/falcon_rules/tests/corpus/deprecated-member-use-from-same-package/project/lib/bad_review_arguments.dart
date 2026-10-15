class Marker {
  const Marker([@deprecated int old = 0]);
}
class NamedMarker {
  const NamedMarker({@deprecated int old = 0});
}
@Marker(1)
@NamedMarker(old: 1)
void annotated() {}
class Callable {
  void call([@deprecated int old = 0]) {}
}
void uses(Callable callable) {
  callable(1);
  callable.call(1); /* expect: deprecated-member-use-from-same-package */
}
@deprecated
typedef OldCallback = void Function();
void callback(OldCallback value) {} /* expect: deprecated-member-use-from-same-package */
