import 'package:flutter/widgets.dart' as f;

class RichText {
  const RichText({dynamic text});
  const RichText.named({dynamic text});
}

class RichTextFactory {
  RichText({dynamic text}) => text;
}

void unrelatedNames() {
  RichText(text: 'local');
  new RichText(text: 'local new');
  const RichText(text: 'local const');
  RichText.named(text: 'local named');
  RichTextFactory().RichText(text: 'method');
}

void shadowedImport(dynamic f) {
  f.RichText(text: 'shadowed prefix');
}

void shadowedLocal() {
  final RichText = ({dynamic text}) => text;
  RichText(text: 'local closure');
}

void unrelatedShorthand() {
  RichText local = .new(text: 'unrelated shorthand');
  RichText named = .named(text: 'unrelated named shorthand');
}
