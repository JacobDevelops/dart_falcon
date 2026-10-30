import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;

void example() {
  Text.rich(const TextSpan(text: 'plain'));
  new Text.rich(const TextSpan(text: 'new'));
  const Text.rich(TextSpan(text: 'const'));
  f.Text.rich(const f.TextSpan(text: 'prefixed'));
  new f.Text.rich(const f.TextSpan(text: 'prefixed new'));
  const f.Text.rich(f.TextSpan(text: 'prefixed const'));
  const Text('simple');
  final constructor = RichText.new;
}

void shadowedConstructor(dynamic RichText) {
  RichText(text: const TextSpan(text: 'not a Flutter constructor'));
}

void shadowedPrefix(dynamic f) {
  f.RichText(text: const TextSpan(text: 'not an import prefix'));
}

void lexicalShadow() {
  final RichText = ({dynamic text}) => text;
  RichText(text: const TextSpan(text: 'local closure shadows Flutter'));
}

class MemberShadow {
  RichText({dynamic text}) => text;

  void example() {
    RichText(text: const TextSpan(text: 'member shadows Flutter'));
  }
}

void preferredShorthand() {
  Text text = .rich(const TextSpan(text: 'shorthand'));
}
