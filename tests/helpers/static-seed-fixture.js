"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

/** Read the companion's wire fixture in place; never evaluate C# or copy source. */
function descriptorBytes() {
  const source = fs.readFileSync(path.resolve(__dirname,
    "../../../TinyRustLM.com/dotnet/tests/TinyRustLM.Companion.Tests/CompanionDirectCompositionSeedDescriptorMaterializerTests.cs"), "utf8");
  const expression = source.match(/private const string RustAcceptedDescriptor =([\s\S]*?);/)[1];
  const literal = /"(?:\\.|[^"\\])*"/g;
  assert.equal(expression.replace(literal, "").replace(/[+\s]/g, ""), "");
  return Buffer.from(expression.match(literal).map((token) => JSON.parse(token)).join(""), "utf8");
}

module.exports = Object.freeze({ descriptorBytes });
