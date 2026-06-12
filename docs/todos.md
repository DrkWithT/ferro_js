### TO DO's:

#### v0.1.0
 - Support more JS operators, including prefix, postfix, bitwise, and logical ops.
    - Prefix and postfix: **DONE**
        - prefix first for `++` and `--`
        - postfix after prefix versions
    - Bitwise: **DONE**
        - Grammar 1: `unary -> ("~" | ...) inner`
        - Grammar 2: `equality & equality` -> `bitand ^ bitand` -> `bitxor | bitxor`
        - `~`, `&`, `|`, `^` --> rule between equality and logical
    - Logical operators `&&`, `||`, `!` **DONE**
        - `&&` or `||` will short circuit evaluate LHS and/or RHS, possibly not boolean
    - Add loose equality: **WIP**
 - Support more control flow: `while`, C-style `for`, `switch`
    - Add `break` and `continue` support.
 - Support global this.
 - Support special impl attributes in global object properties: `IsDecl` must be checked on `delete`.
 - Support property semantics: data vs. accessor, writable + configurable + enumerable.
 - Support simple functions, including their call-this semantics.
 - Support native display(...args) function.

#### v0.2.0
 - Support compound assignment ops: `*=`, `/=`, `+=`, `-=`
 - Add `typeof`, `delete`
 - Support built-in `Date`, `Math`.
 - Support built-in `Array`, `Object`.
    - Support array (native object with array prototype)
    - Support array literals.
    - Support array prototype methods except `sort` and locales.
