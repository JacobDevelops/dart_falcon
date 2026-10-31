import 'support.dart';
const plain = 'assets/plain.png';
const map = 'assets/map.png';
const raw = r'assets/raw$dollar.png';
const adjacent = 'assets/' 'adjacent.png';
String dynamicAsset(String name) => 'assets/dynamic/$name.png';
String prefix(String name) => 'assets/${name}.png';
