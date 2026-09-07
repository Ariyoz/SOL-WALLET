/**
 * bs58.js — Base58 encode/decode (Bitcoin alphabet)
 * Self-contained, no dependencies. Exposed as window.bs58.
 * Compatible with Solana keypair format.
 */
(function (root) {
  'use strict';
  var ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  var BASE = 58;
  var LEADER = ALPHABET[0];
  var FACTOR = Math.log(BASE) / Math.log(256);
  var iFACTOR = Math.log(256) / Math.log(BASE);

  var alphabetMap = {};
  for (var i = 0; i < ALPHABET.length; i++) alphabetMap[ALPHABET[i]] = i;

  function encode(source) {
    if (source.length === 0) return '';
    var digits = [0];
    for (var i = 0; i < source.length; i++) {
      var carry = source[i];
      for (var j = 0; j < digits.length; j++) {
        carry += digits[j] << 8;
        digits[j] = carry % BASE;
        carry = (carry / BASE) | 0;
      }
      while (carry > 0) { digits.push(carry % BASE); carry = (carry / BASE) | 0; }
    }
    var str = '';
    for (var k = 0; source[k] === 0 && k < source.length - 1; k++) str += LEADER;
    for (var m = digits.length - 1; m >= 0; m--) str += ALPHABET[digits[m]];
    return str;
  }

  function decode(str) {
    if (str.length === 0) return new Uint8Array(0);
    var bytes = [0];
    for (var i = 0; i < str.length; i++) {
      var value = alphabetMap[str[i]];
      if (value === undefined) throw new Error('Non-base58 character: ' + str[i]);
      var carry = value;
      for (var j = 0; j < bytes.length; j++) {
        carry += bytes[j] * BASE;
        bytes[j] = carry & 0xff;
        carry >>= 8;
      }
      while (carry > 0) { bytes.push(carry & 0xff); carry >>= 8; }
    }
    for (var k = 0; str[k] === LEADER && k < str.length - 1; k++) bytes.push(0);
    var result = new Uint8Array(bytes.reverse());
    return result;
  }

  root.bs58 = { encode: encode, decode: decode };
})(window);
