import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void fields(Object? controller, Function callback) {
  TextField(controller: controller);
  TextFormField(onChanged: callback);
  f.CupertinoTextField.borderless(onSubmitted: callback);
  EditableText(onChanged: callback);
  TextFormField(onSaved: callback);
  TextFormField(onFieldSubmitted: callback);
  TextField(controller: TextEditingController());
  TextField(readOnly: true);
  TextFormField(enabled: false);
  CupertinoTextField.borderless(readOnly: true);
  EditableText(readOnly: true);
}
void shadow(dynamic TextField) { TextField(); }
