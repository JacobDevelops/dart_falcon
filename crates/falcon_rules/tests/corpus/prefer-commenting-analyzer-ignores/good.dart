// Legacy API retained for compatibility.
// ignore_for_file: deprecated_member_use

// Needed for a platform-specific implementation.
// ignore: unused_import
import 'dart:math';

// ignore: unused_element // Called by the platform through reflection.
void reflected() {}

/* The generated visitor requires this hook. */
// ignore: unused_element
void hook() {}

/// Entry point invoked externally.
// ignore: unused_element
void entryPoint() {}

// ignore: unused_element This entry point is invoked externally.
void inlineProse() {}

// ignore: unused_element, long-method This generated hook needs both exceptions.
void multipleCodes() {}

// ignore: type=lint Compatibility shim maintained for an external caller.
void allLintProse() {}

const ordinary = '// ignore: unused_element';
const raw = r'// ignore_for_file: unused_element';
const multiline = '''
// ignore: unused_element
''';
const rawMultiline = r'''
// ignore_for_file: unused_element
''';
/* // ignore: unused_element */
// This is prose mentioning // ignore: unused_element, not a directive.
