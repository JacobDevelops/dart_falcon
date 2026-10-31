import 'support.dart';
const font = 'assets/demo.ttf';
const shader = 'assets/shader.frag';
const deferred = 'assets/deferred.png';
void main() { image('assets/brand.png', package: 'brand'); }
void named() { load(asset: 'assets/brand_named.png', package: 'brand'); }
void load({String? asset, String? package}) {}
