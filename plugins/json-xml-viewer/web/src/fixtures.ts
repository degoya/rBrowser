// Test helpers shared by the suites; never bundled (main.ts does not import it).

import { XmlNode } from './xml-nodes';

/** The tree roots of an XML text, parsed as the engine would. */
export function xmlRoots(text: string): XmlNode[] {
  const doc = new DOMParser().parseFromString(text, 'application/xml');
  return XmlNode.roots([...doc.childNodes]);
}
