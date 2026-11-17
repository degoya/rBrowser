// The plain-text search: every node whose name or scalar value contains the query, without
// regard to case, in document order. An XML element matches by its tag, its attributes and the
// text it shows inline. It walks the whole document (iteratively, so depth cannot overflow it)
// and stops at the result limit.

import type { TreeNode } from './nodes';

/** The most nodes a search answers. */
export const MAX_MATCHES = 10000;

export interface Matches {
  nodes: TreeNode[];
  truncated: boolean;
}

export function searchNodes(roots: readonly TreeNode[], query: string, limit = MAX_MATCHES): Matches {
  const needle = query.toLowerCase();
  const nodes: TreeNode[] = [];
  const stack: TreeNode[] = [...roots].reverse();
  while (stack.length > 0) {
    const node = stack.pop() as TreeNode;
    if (node.searchText().toLowerCase().includes(needle)) {
      if (nodes.length >= limit) return { nodes, truncated: true };
      nodes.push(node);
    }
    const kids = node.children();
    for (let i = kids.length - 1; i >= 0; i -= 1) stack.push(kids[i] as TreeNode);
  }
  return { nodes, truncated: false };
}
