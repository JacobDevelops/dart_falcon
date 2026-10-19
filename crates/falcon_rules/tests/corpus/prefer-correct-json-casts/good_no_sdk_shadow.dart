import 'dart:convert';
import 'dart:convert' as convert show jsonDecode;

class JsonDecoder {
  dynamic convert(String text) => <int>[1];
}

class Codec {
  dynamic decode(String text) => <int>[1];
}

final json = Codec();

void decode(String text) {
  final codec = json.decode(text) as List<int>;
  final decoder = JsonDecoder().convert(text) as List<int>;
  final explicitDecoder = new JsonDecoder().convert(text) as List<int>;
  final hiddenCodec = convert.json.decode(text) as List<int>;
  final hiddenDecoder = convert.JsonDecoder().convert(text) as List<int>;
}
