// pyramid_lint only inspects variable/field declarations initialised with a
// boolean *literal*, and bool-returning methods/getters/functions. Parameters,
// uninitialised fields, and non-literal initialisers are all out of scope.

class Flags {
  bool isActive = true;
  bool hasPermission = false;
  bool canEdit = true;
  bool shouldRefresh = false;
  bool _isDisposed = false; // leading underscore stripped -> isDisposed

  // Uninitialised bool fields have no boolean literal, so they are not checked.
  final bool active;
  late bool loading;

  // A non-literal initialiser (`!kDebugMode`) is out of scope.
  bool enabled = !kDebugMode;

  Flags(this.active);

  // Parameters are never checked, even with a boolean-literal default.
  void configure(bool ready, {bool fatal = false}) {
    print(ready);
    print(fatal);
    bool isDone = true; // local with a valid prefix
    print(isDone);
  }

  bool get isEmpty => !isActive;
  bool hasValue() => isActive;
}

// An @override method inherits its name from the supertype and is exempt.
class Sub extends Flags {
  Sub() : super(false);

  @override
  bool validate() => true;
}
