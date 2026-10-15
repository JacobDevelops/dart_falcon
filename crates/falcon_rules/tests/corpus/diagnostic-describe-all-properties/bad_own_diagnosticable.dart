// Inside the Flutter framework `Diagnosticable` is a project declaration; any
// supertype named `Diagnosticable` from an unnamed library counts.
mixin Diagnosticable {}

class Node with Diagnosticable {
  Node(this.label);

  final String label; /* expect: diagnostic-describe-all-properties */
}
