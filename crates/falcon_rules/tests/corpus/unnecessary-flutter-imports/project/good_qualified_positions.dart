import 'package:flutter/widgets.dart' as objectPattern;
import 'package:flutter/widgets.dart' as recordPattern;
import 'package:flutter/widgets.dart' as listPattern;
import 'package:flutter/widgets.dart' as mapPattern;
import 'package:flutter/widgets.dart' as typeTest;
import 'package:flutter/widgets.dart' as typeCast;
import 'package:flutter/widgets.dart' as typeLiteral;
import 'package:flutter/widgets.dart' as extensionApplication;
import 'package:flutter/widgets.dart' as enumValue;
import 'package:flutter/widgets.dart' as metadata;
import 'package:flutter/widgets.dart' as explicitNew;
import 'package:flutter/widgets.dart' as implicitNew;
import 'package:flutter/widgets.dart' as unnamedTearoff;
import 'package:flutter/widgets.dart' as namedTearoff;
import 'package:flutter/widgets.dart' deferred as delayed show FrameworkBox;
import 'package:flutter/widgets.dart' as switchCase;
import 'package:flutter/widgets.dart' as nested;

@metadata.FrameworkBox<List<nested.Widget>>.named([])
class Annotated {}

bool object(Object value) => switch (value) {
  objectPattern.FrameworkBox<List<nested.Widget>>(:final field) => field.isEmpty,
  _ => false,
};
bool record(Object value) => switch (value) {
  (recordPattern.FrameworkBox<List<nested.Widget>>(),) => true,
  _ => false,
};
bool list(Object value) => switch (value) {
  <listPattern.FrameworkBox<List<nested.Widget>>>[] => true,
  _ => false,
};
bool map(Object value) => switch (value) {
  <String, mapPattern.FrameworkBox<List<nested.Widget>>>{'value': _} => true,
  _ => false,
};
bool test(Object value) => value is typeTest.FrameworkBox<List<nested.Widget>>;
Object cast(Object value) => value as typeCast.FrameworkBox<List<nested.Widget>>;
Object literal() => typeLiteral.FrameworkBox<List<nested.Widget>>;
Object extension() => extensionApplication.FrameworkBoxExtension<List<nested.Widget>>(
  const extensionApplication.FrameworkBox<List<nested.Widget>>([]),
).value;
Object enumConstant() => enumValue.WidgetKind.first;
final created = new explicitNew.FrameworkBox<List<nested.Widget>>.named([]);
final implicit = implicitNew.FrameworkBox<List<nested.Widget>>.named([]);
final unnamed = unnamedTearoff.FrameworkBox<List<nested.Widget>>.new;
final named = namedTearoff.FrameworkBox<List<nested.Widget>>.named;
Future<Object> deferred() async {
  await delayed.loadLibrary();
  return delayed.FrameworkBox<List<nested.Widget>>.named([]);
}
bool caseConstant(Object value) {
  switch (value) {
    case switchCase.WidgetKind.first:
      return true;
    default:
      return false;
  }
}
