// test cond ternary exprs

var a = 1, b = -1, c = 0, ok = 0;

if (a ? b : c === -1) {
    ok++;
}

if (c ? a : b === 1) {
    ok++;
}

// should set b as 0
b < c ? b = c : b++;

if (b === 0) {
    ok++;
}

return ok === 3;
