#!/usr/bin/env bash

set -e

mkdir -p temp
pushd temp > /dev/null

npm i --loglevel=error babylon@next > /dev/null

echo "
(function() {
var exports = {};
$(cat ./node_modules/babylon/lib/index.js)
function parse_to_string(code) {
	return JSON.stringify(exports.parse(code));
}
return parse_to_string;
})();
" > ../babylon.js

popd > /dev/null
rm -r temp
