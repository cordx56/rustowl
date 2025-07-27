import {
  zLspCursorResponse,
  zLspRange,
  zLspLocation,
  zLspType,
  zAliveMessage,
  zIndex,
} from "../src/schemas.js";
import assert from "node:assert";
import { describe, it } from "mocha";

describe("Schema Tests", () => {
  it("should validate a valid zAliveMessage", () => {
    const data = { status: true };
    assert.doesNotThrow(() => zAliveMessage.parse(data));
  });

  it("should not validate an invalid zAliveMessage", () => {
    const data = { status: false };
    assert.throws(() => zAliveMessage.parse(data));
  });

  it("should validate a valid zIndex", () => {
    const data = 1;
    assert.doesNotThrow(() => zIndex.parse(data));
  });

  it("should not validate an invalid zIndex", () => {
    const data = 1.5;
    assert.throws(() => zIndex.parse(data));
  });

  it("should validate a valid zLspLocation", () => {
    const data = { line: 1, character: 1 };
    assert.doesNotThrow(() => zLspLocation.parse(data));
  });

  it("should not validate an invalid zLspLocation", () => {
    const data = { line: 1.5, character: 1 };
    assert.throws(() => zLspLocation.parse(data));
  });

  it("should validate a valid zLspRange", () => {
    const data = {
      start: { line: 1, character: 1 },
      end: { line: 1, character: 5 },
    };
    assert.doesNotThrow(() => zLspRange.parse(data));
  });

  it("should validate a valid zLspType", () => {
    assert.doesNotThrow(() => zLspType.parse("lifetime"));
    assert.doesNotThrow(() => zLspType.parse("imm_borrow"));
    assert.doesNotThrow(() => zLspType.parse("mut_borrow"));
    assert.doesNotThrow(() => zLspType.parse("move"));
    assert.doesNotThrow(() => zLspType.parse("call"));
    assert.doesNotThrow(() => zLspType.parse("shared_mut"));
    assert.doesNotThrow(() => zLspType.parse("outlive"));
  });

  it("should not validate an invalid zLspType", () => {
    assert.throws(() => zLspType.parse("invalid_type"));
  });

  it("should validate a valid zLspCursorResponse", () => {
    const data = {
      is_analyzed: true,
      status: "finished",
      decorations: [
        {
          type: "lifetime",
          range: {
            start: { line: 1, character: 1 },
            end: { line: 1, character: 5 },
          },
          hover_text: "hover text",
          overlapped: false,
        },
      ],
    };
    assert.doesNotThrow(() => zLspCursorResponse.parse(data));
  });

  it("should not validate an invalid zLspCursorResponse", () => {
    const data = {
      is_analyzed: true,
      status: "invalid_status",
      decorations: [],
    };
    assert.throws(() => zLspCursorResponse.parse(data));
  });
});
