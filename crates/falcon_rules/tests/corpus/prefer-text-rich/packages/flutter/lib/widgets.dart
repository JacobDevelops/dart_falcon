class Widget { const Widget(); }
class InlineSpan { const InlineSpan(); }
class TextSpan extends InlineSpan { const TextSpan({String? text, List<InlineSpan>? children}); }
class RichText extends Widget { const RichText({required InlineSpan text}); }
class Text extends Widget { const Text(String data); const Text.rich(InlineSpan textSpan); }
