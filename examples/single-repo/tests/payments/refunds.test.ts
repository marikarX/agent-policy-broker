import { createRefund } from "../../src/payments/refunds";

test("deduplicates refund requests", () => {
  const request = { chargeId: "ch_123", amountCents: 1200, requestId: "req_1" };

  expect(createRefund(request)).toBe("refund:ch_123:1200");
  expect(createRefund(request)).toBe("duplicate");
});
