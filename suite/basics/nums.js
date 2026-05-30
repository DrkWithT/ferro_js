var a = 42;
var b = 0x2A;
var c = 0b101010;
var n = NaN;
var ok = 0;

if (a === b) {
    ok++;
}

if (b === c) {
    ok++;
}

return ok === 2;
