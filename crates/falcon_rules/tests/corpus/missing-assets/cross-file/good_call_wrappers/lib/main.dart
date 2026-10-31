import 'support.dart';
void use(bool dark) {
  image(dark ? 'assets/dark.png' : 'assets/light.png', package: 'brand');
  image(('assets/parenthesized.png'), package: 'brand');
}
