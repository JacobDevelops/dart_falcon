const legacy = 1;
class Marker {
  const Marker.old();
}
@legacy /* expect: avoid-banned-annotations */
class Example {}
@Marker.old() /* expect: avoid-banned-annotations */
void example() {}
