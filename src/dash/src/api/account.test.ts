// import { describe, expect, it } from "vitest";
// import { proposeTransaction } from "./account";
// import { Principal } from "@dfinity/principal";

// describe("Intent creation", () => {
//   it("creates correct intent object for ICP", () => {
//     const createIntent = (
//       amount: string,
//       token: string,
//       recipient: string,
//       nativeAccountId: string
//     ): {
//       amount: number;
//       token: string;
//       to: string;
//       network: { ETH: null } | { ICP: null };
//       transaction_type: { Transfer: null };
//       from: string;
//     } => ({
//       amount: Number(amount),
//       token,
//       to: recipient,
//       network: token.toLowerCase().includes("eth")
//         ? { ETH: null }
//         : { ICP: null },
//       transaction_type: { Transfer: null },
//       from: nativeAccountId,
//     });

//     const proposal = createIntent(
//       "100",
//       "icp:native",
//       "recipient",
//       "account-id"
//     );

//     expect(proposal).toEqual({
//       amount: 100,
//       token: "icp:native",
//       to: "recipient",
//       network: { ICP: null },
//       transaction_type: { Transfer: null },
//       from: "account-id",
//     });

//     const proposedTransaction = proposeTransaction(
//       Principal.fromText("account-id"),
//       proposal,
//       identity!
//     );
//   });
// });
