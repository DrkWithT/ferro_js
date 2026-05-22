var a = 42;
var b = 0x2A;
var c = 0b101010;
var n = NaN;

if (a !== b || b !== c) {
    return false;
}

// NaN cannot equal itself.
if (n === n) {
    return false;
}

return true;
