class Api {
  @Deprecated('Use newMethod instead.')
  void oldMethod() {}

  @Deprecated('Removed in version 2.0.')
  int oldField = 0;

  @override
  String toString() => 'Api';
}

@Deprecated('No longer supported; use Service.')
void oldTopLevel() {}

@Deprecated('Use the replacement class.')
class OldClass {}
