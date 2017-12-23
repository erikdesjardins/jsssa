#!/usr/bin/env bash

set -e

mkdir -p temp
pushd temp > /dev/null

npm i acorn &> /dev/null

echo "var module = {}, exports = {}, acorn = exports; $(cat ./node_modules/acorn/dist/acorn.js ./node_modules/acorn/bin/acorn)" |
sed "/require('..\/dist\/acorn.js')/d ; /^'use strict'/d ; /#!\//d" |
tee ../acorn.js > /dev/null

popd > /dev/null
rm -r temp
