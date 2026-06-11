// test prefix + operator

var a = 1, b = -1, ok = 0;
//var c = "42", d = "test";

if (+a === 1) {
    ok++;
}

if (+b === -1) {
    ok++;
}

if (+undefined !== NaN) {
    ok++;
}

if (+null === 0) {
    ok++;
}

// if (+c === 42) {
//     ok++;
// }

// if (+d !== NaN) {
//     ok++;
// }

return ok === 4;
