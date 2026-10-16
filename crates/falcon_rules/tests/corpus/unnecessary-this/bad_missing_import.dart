// An unresolved import supplies no element to the SDK linter's name lookup.
import 'package:missing/missing.dart' show hidden;

class MissingImportBase {
  int hidden = 1;
}

class MissingImportInherited extends MissingImportBase {
  int read() => this.hidden; /* expect: unnecessary-this */
  int shadowed(int hidden) => this.hidden + hidden;
}
