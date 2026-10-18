import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void fields() {
  TextField(); /* expect: avoid-missing-controller */
  TextFormField(); /* expect: avoid-missing-controller */
  f.CupertinoTextField.borderless(); /* expect: avoid-missing-controller */
  new EditableText(controller: null); /* expect: avoid-missing-controller */
  TextField(controller: null, onChanged: null); /* expect: avoid-missing-controller */
  TextFormField(onSaved: null); /* expect: avoid-missing-controller */
}
