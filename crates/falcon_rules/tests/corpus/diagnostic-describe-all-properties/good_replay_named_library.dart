library named;
mixin Diagnosticable {}
class Node with Diagnosticable {
  Node(this.label);
  final String label;
}
