var foo = {
    bar: {
        a: 69,
        b: null
    }
};

var barInfo = Object.getOwnPropertyDescriptor(foo, "bar");
barInfo.configurable = false;
barInfo.writable = false;

if (delete foo.bar.b === true) {
    return false;
}

foo.bar = null;

if (foo.bar === null) {
    return false;
}

return true;
