export type RefundRequest = {
  chargeId: string;
  amountCents: number;
  requestId: string;
};

const processedRequestIds = new Set<string>();

export function createRefund(request: RefundRequest): string {
  if (processedRequestIds.has(request.requestId)) {
    return "duplicate";
  }

  processedRequestIds.add(request.requestId);
  return `refund:${request.chargeId}:${request.amountCents}`;
}
