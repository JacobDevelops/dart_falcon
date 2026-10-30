import 'package:flutter/widgets.dart';
import 'package:flutter/material.dart' as f;

typedef RichTextAlias = RichText;

void example() {
  RichText(text: const TextSpan(text: 'plain')); /* expect: prefer-text-rich, msg: "Use `Text.rich` instead of `RichText`." */
  new RichText(text: const TextSpan(text: 'new')); /* expect: prefer-text-rich */
  const RichText(text: TextSpan(text: 'const')); /* expect: prefer-text-rich */
  f.RichText(text: const f.TextSpan(text: 'prefixed')); /* expect: prefer-text-rich */
  new f.RichText(text: const f.TextSpan(text: 'prefixed new')); /* expect: prefer-text-rich */
  const f.RichText(text: f.TextSpan(text: 'prefixed const')); /* expect: prefer-text-rich */
  RichTextAlias(text: const TextSpan(text: 'alias')); /* expect: prefer-text-rich */
  RichText.new(text: const TextSpan(text: 'explicit unnamed')); /* expect: prefer-text-rich */
  f.RichText.new(text: const f.TextSpan(text: 'prefixed explicit unnamed')); /* expect: prefer-text-rich */
  RichText shorthand = .new(text: const TextSpan(text: 'shorthand')); /* expect: prefer-text-rich */
  RichTextAlias aliasShorthand = .new(text: const TextSpan(text: 'alias shorthand')); /* expect: prefer-text-rich */
  const RichText constantShorthand = .new(text: TextSpan(text: 'const shorthand')); /* expect: prefer-text-rich */
}
