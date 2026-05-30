// test filled vars
var a = 1, b = 2, ok = 0;

if (a === 1 || b === 2) {
    ok++;
}

// test dud vars
var c, d;

if (c === undefined || d === undefined) {
    ok++;
}

return ok == 2;
