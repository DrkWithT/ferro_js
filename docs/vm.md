### DerkJS VM

#### Basics
 - Stack-based value passing
    - Temporary stack
    - Environments stack / chain
    - Completion record stack??
 - Match opcode & loop dispatch for standard Rust support
 - String interning
 - Inline Caches + Shape system
 - In-place value ops
 - Mark & sweep GC (PLANNED)

#### Call Layout on Stack:
```
EXAMPLE: call of foo(1, 2):
-----------------
TEMP STACK, pre-call:
| ......... | <-- UNFILLED ARGS become UNDEFINED!
| Value(2)  | <-- STACK[SP]
| Value(1)  |
| ref(foo)  | <-- STACK[CALLEE_BP] uses CALLEE_BP = SP - ARGC
| ref?this  | <-- STACK[CALLEE_BP - 1] is `thisArg`, defaulted to `globalThis`.
-----------------
```
##### Begin call:
1. Load array-like object viewing N temp args to Environment.arguments. A new environment is created if:
   - Case 1: Global scope / code is first entered.
   - Case 2: Constructor calls make a new environment object for `this`.
   - Case 3: If a function captures foreign names or has captured names.
      - Functions are checked at compile time for "foreign" captured names... If there's none, the function is marked "plain", not requiring an environment object itself (but a passed closure argument will!)
      - Bind `this` to global environment IFF the call is regular.
-----------------
##### End call:
1. Pop environment IFF there's no function returned, leaving a completion record with the return value. Otherwise, a closure is returned in the record.
2. The destination of the callee's result gets the unpacked `[[Value]]`.
##### Native call:
1. The native callee _must_ return its result value at `CALLEE_RESULT_SLOT = SP - ARGC - 1` & set SP accordingly.
2. Trampolined from bytecode to native via `[NATIVE_CALL, RET]`

#### Normal Opcodes
 - PUSH_UNDEF
 - PUSH_NULL
 - PUSH_BOOL
 - PUSH_NAN
 - PUSH_INF
 - PUSH_NEG_INF
 - PUSH_CONST
 - DUP1        NOTE: a -> a a^
 - DUP2        NOTE: a b -> a a b^
 - SWAP        NOTE: a b -> b a^
 - GET_LOCAL   NOTE: only used if no environment object is needed.
 - SET_LOCAL
 - GET_VAR     NOTE: tries getting a var from the callee's environment object, etc.
 - SET_VAR
 - MAKE_OBJ
 - GET_OWN_PROP
 - SET_OWN_PROP
 - GET_PROP    NOTE: If generic flag (aka [] access) is on, do so.
 - SET_PROP    NOTE: If generic flag (aka [] access) is on, do so.
 - DEL_PROP    NOTE: ignore indexed items for now!
 - GET_PROTO   NOTE: If hidden flag is on: get `[[Prototype]]`
 - SET_PROTO   NOTE: If hidden flag (`0b0001`) is on: set `[[Prototype]]`. If hidden flag 2 (`0b0010`) is on: use a built-in prototype Value, but use the stack top otherwise.
 - TO_BOOLEAN
 - TO_NUMBER
 - INC_LOCAL N    NOTE: increments top stack value at BP + N -- _Prefix gives newValue BUT postfix gives the oldValue!_
 - DEC_LOCAL N    NOTE: decrements top stack value at BP + N
 - INC_PROP G     NOTE: requires a key and the object below on the stack.
 - DEC_PROP G     NOTE: requires a key and the object below on the stack.
 - NEGATE_BOOL
 - NEGATE_NUM
 - MOD
 - MUL
 - DIV
 - ADD
 - SUB
 - BT_FLIP
 - BT_AND
 - BT_OR
 - BT_XOR
 - STRICT_EQ
 - STRICT_NE
 - LOOSE_EQ
 - LOOSE_NE
 - LT
 - LTE
 - GT
 - GTE
 - JUMP_IF
 - JUMP_ELSE
 - JUMP
 - CALL           NOTE: This Fun args\[n\]^ -> Result^
 - CALL_CTOR      NOTE: _Unimplemented_ since objects and prototypes need implementations!
 - NATIVE_CALL    NOTE: takes the ID of a native function pointer from its buffer & arg-count.
 - RET
 - RET_CLOSURE ?

#### Super Opcodes ?
 - ADD_K
 - SUB_K
 - 
