import 'support.dart';
import 'assets/import.dart';
const missing = 'assets/missing.png'; /* expect: missing-assets, msg: "Asset `assets/missing.png` is not declared under `flutter: assets:` in `pubspec.yaml`." */
const nested = 'assets/icons/sub/deep.png'; /* expect: missing-assets */
const adjacent = 'assets/' 'joined.png'; /* expect: missing-assets */
const raw = r'assets/raw.png'; /* expect: missing-assets */
