export type Piece = { type: "same" | "add" | "del" | "tweak"; text: string; was?: string };

const norm = (t: string) => t.toLowerCase().replace(/[^\p{L}\p{N}']/gu, "");

/**
 * Word-level diff of what you said vs. what got typed.
 * "tweak" = same word, different punctuation or capitalization.
 */
export function diffWords(a: string, b: string): Piece[] {
  const A = a.trim().split(/\s+/).filter(Boolean);
  const B = b.trim().split(/\s+/).filter(Boolean);
  const n = A.length;
  const m = B.length;
  if (n * m > 4_000_000) return [{ type: "same", text: b }];

  const eq = (i: number, j: number) => {
    const x = norm(A[i]);
    return x ? x === norm(B[j]) : A[i] === B[j];
  };
  const dp = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--)
      dp[i][j] = eq(i, j) ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);

  const out: Piece[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (eq(i, j)) {
      out.push(A[i] === B[j] ? { type: "same", text: B[j] } : { type: "tweak", text: B[j], was: A[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) out.push({ type: "del", text: A[i++] });
    else out.push({ type: "add", text: B[j++] });
  }
  while (i < n) out.push({ type: "del", text: A[i++] });
  while (j < m) out.push({ type: "add", text: B[j++] });

  const merged: Piece[] = [];
  for (const p of out) {
    const last = merged[merged.length - 1];
    if (last && last.type === p.type && p.type !== "tweak") last.text += " " + p.text;
    else merged.push({ ...p });
  }
  return merged;
}
