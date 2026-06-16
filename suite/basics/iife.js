// test a JS IIFE mess

return (function(x) {
    return (function (y) {
        return x + y;
    })(10);
})(10);
