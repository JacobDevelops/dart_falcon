// No expectations on purpose: this file has no enclosing pubspec.yaml, so the
// rule cannot tell which package it belongs to and stays silent.
// project/lib/bad_imports.dart and project/lib/src/bad_nested.dart cover the
// reporting cases.
import 'package:app/src/a.dart';

void main() {}
