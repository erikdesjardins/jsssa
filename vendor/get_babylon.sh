#!/usr/bin/env bash

set -e

mkdir -p temp
pushd temp > /dev/null

npm i --loglevel=error babylon@next > /dev/null

echo "
var exports = {};
$(cat ./node_modules/babylon/lib/index.js)
var code = '';
process.stdin.resume();
process.stdin.on('data', function(s) { code += s; });
process.stdin.on('end', function() {
	process.stdout.write(JSON.stringify(exports.parse(code)));
});
" > ../babylon.js

popd > /dev/null
rm -r temp
