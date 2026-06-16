// test filled vars
var a = 1, b, ok = 0;

if (a === 1) {
    ok++;
}

if (b === undefined) {
    ok++;
}

return ok === 2;
