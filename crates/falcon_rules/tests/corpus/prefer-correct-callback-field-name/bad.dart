typedef Callback = void Function();
class Callbacks {
  final void Function() tap; /* expect: prefer-correct-callback-field-name */
  void Function(int)? _changed; /* expect: prefer-correct-callback-field-name */
  Function? work; /* expect: prefer-correct-callback-field-name */
  Callback? alias; /* expect: prefer-correct-callback-field-name */
  String onValue = ''; /* expect: prefer-correct-callback-field-name */
  Callbacks(this.tap);
}
