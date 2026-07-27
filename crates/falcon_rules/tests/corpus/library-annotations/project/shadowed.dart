@NotLibraryTargeted()
import 'annotations.dart' show LibraryOnly;

class Target {
  const Target(Object kinds);
}

class TargetKind {
  static const library = Object();
}

@Target({TargetKind.library})
class NotLibraryTargeted {
  const NotLibraryTargeted();
}

void run() {}
