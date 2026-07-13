import assert from "node:assert/strict";
import test from "node:test";
import { isPermanentlyInvalidTokenError } from "./notification-delivery";

test("classifies invalid provider tokens as permanent", () => {
  assert.equal(
    isPermanentlyInvalidTokenError({
      code: "messaging/registration-token-not-registered",
    }),
    true,
  );
  assert.equal(
    isPermanentlyInvalidTokenError({ code: "messaging/internal-error" }),
    false,
  );
  assert.equal(isPermanentlyInvalidTokenError(undefined), false);
});
