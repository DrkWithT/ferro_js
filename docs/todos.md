### TO DO's:

#### v0.1.0
 - Support more JS operators, including prefix, postfix, bitwise, and logical ops.
 - Support simple functions.
 - Support simple objects with property semantics: data vs. accessor.
 - Implement closures.
 - Support relational ops, bitshifts, conditional ternary op, assignment op, comma.
   - Grammar:
      - `<conditional> = <or> ( "?" <assign> ":" <assign> )?`
      - `<assign> = <arrow-function> | <conditional> ( "=" <assign> )?`
      - `<expr-stmt> = <assign> ";"`
 - Add `typeof`, `delete`
   - TypeOf results:
      - Undefined: `"undefined"`
      - Null: `"object"`
      - Boolean: `"boolean"`
      - Number: `"number"`
      - String: `"string"`
      - ~~Symbol: `"symbol"`~~
      - non-Function Object: `"object"`
      - functions: `"function"`
   - Delete result:
      - The operation succeeds with `true` when property is non-accessor AND configurable.
      - NOTE: writable + configurable + enumerable are 3 important flags per property to check at runtime.
         - Maybe Shapes should store this metadata.
 - Support strings.
 - Refactor codebase into modular, Dep. Injected driver.

#### v0.2.0
 - Support intrinsics for future:
   - Support generation of mappings to intrinsic objects.
   - Add pool for "immortal" and irremovable objects e.g `Date`, `Math`, `Boolean`, `Number`, `String`, `Array`, `Object`, and possibly `Promise`.
 - Support `Function` semantics e.g `.name, .length, .constructor, .call()`
 - Support native display(...args) function.
    - Trampoline to native calls with stub array that has `[PushUndef, NativeCall, Ret]`.
 - Add `break` and `continue` support.
 - Support more control flows: C-style `for`, `switch`
 - Handle computed property keys with caching them by string -> existing ID...
 - Support built-in `Date`, `Math`.
 - Support built-in `Array`, `Object`.
    - Support array (native object with array prototype)
    - Support array literals.
    - Support array prototype methods except `sort` and locales.
 - Support compound assignment ops: `*=`, `/=`, `+=`, `-=`
