/// @docImport 'scope_docs.dart';
library;

import 'scope_first.dart';
import 'scope_second.dart';

class ScopeBase {
  int importedName() => 1;
  int documentedName() => 2;
}

class InheritedScope extends ScopeBase {
  int readImported() => this.importedName();
  int readDocumented() => this.documentedName(); /* expect: unnecessary-this */
}

class DeclaredScope {
  int importedName() => 1;
  int readImported() => this.importedName(); /* expect: unnecessary-this */
}
