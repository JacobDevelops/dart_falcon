export 'src/widgets/framework.dart';
import 'src/widgets/framework.dart';
class Hero extends Widget { const Hero({Key? key, Object? tag, Widget? child}); }
class Text extends Widget { const Text(String data); }
class Icon extends Widget { const Icon(Object? icon, {String? semanticLabel}); }
class IconButton extends Widget {
  const IconButton({Key? key, Widget? icon, String? tooltip, Function? onPressed});
  const IconButton.filled({Key? key, Widget? icon, String? tooltip, Function? onPressed});
}
class FloatingActionButton extends Widget {
  const FloatingActionButton({Key? key, Widget? child, String? tooltip, Function? onPressed});
  const FloatingActionButton.small({Key? key, Widget? child, String? tooltip, Function? onPressed});
}
class GestureDetector extends Widget { const GestureDetector({Key? key, Widget? child, Function? onTap, bool excludeFromSemantics = false}); }
class InkWell extends Widget { const InkWell({Key? key, Widget? child, Function? onTap, bool excludeFromSemantics = false}); }
class InkResponse extends Widget { const InkResponse({Key? key, Widget? child, Function? onTap, bool excludeFromSemantics = false}); }
class Tooltip extends Widget { const Tooltip({String? message, Widget? child}); }
class Semantics extends Widget { const Semantics({String? label, Widget? child}); }
class SizedBox extends Widget { const SizedBox({double? width, double? height, Widget? child}); }
class Container extends Widget { Container({double? width, double? height, Widget? child}); }
class TextButton extends Widget { const TextButton({Widget? child, Function? onPressed}); }
class ElevatedButton extends Widget { const ElevatedButton({Widget? child, Function? onPressed}); }
class OutlinedButton extends Widget { const OutlinedButton({Widget? child, Function? onPressed}); }
class RawMaterialButton extends Widget { const RawMaterialButton({Widget? child, Function? onPressed}); }
class CupertinoButton extends Widget { const CupertinoButton({Widget? child, Function? onPressed}); }
class TextEditingController {}
class TextField extends Widget { TextField({Object? controller, Function? onChanged, Function? onSubmitted}); }
class TextFormField extends Widget { TextFormField({Object? controller, Function? onChanged, Function? onSaved, Function? onFieldSubmitted}); }
class CupertinoTextField extends Widget {
  CupertinoTextField({Object? controller, Function? onChanged, Function? onSubmitted});
  CupertinoTextField.borderless({Object? controller, Function? onChanged, Function? onSubmitted});
}
class EditableText extends Widget { EditableText({Object? controller, Function? onChanged, Function? onSubmitted}); }
class FormState extends State<Widget> {}
class Form extends Widget { Form({Key? key, Widget? child}); }
