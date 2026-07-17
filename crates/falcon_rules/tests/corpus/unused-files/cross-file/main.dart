// Entrypoint: has main() and references the other live files, so nothing here
// is flagged and the files it imports count as used.
import 'used.dart';
import 'feature.dart';

void main() {
  helper();
  runFeature();
}
