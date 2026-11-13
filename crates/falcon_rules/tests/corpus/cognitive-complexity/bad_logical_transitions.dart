bool alternating(bool a, bool b, bool c, bool d) => a || b && c || d; /* expect: cognitive-complexity, msg: "Function has cognitive complexity 3 (max 2). Reduce nesting or extract helpers." */
bool longer(bool a, bool b, bool c, bool d, bool e) => a && b || c && d || e; /* expect: cognitive-complexity, msg: "Function has cognitive complexity 4 (max 2). Reduce nesting or extract helpers." */
bool grouped(bool a, bool b, bool c, bool d) => (a || b) && (c || d); /* expect: cognitive-complexity, msg: "Function has cognitive complexity 3 (max 2). Reduce nesting or extract helpers." */
