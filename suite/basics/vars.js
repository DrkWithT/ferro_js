// test filled vars
var a = 1, b = 2;

if (a !== 1 || b !== 2) {
    return false;
}

// test dud vars
var c, d;

if (c !== undefined || d !== undefined) {
    return false;
}

return true;
