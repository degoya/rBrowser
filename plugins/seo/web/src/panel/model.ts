// The SEO panel's data (RB-080-11, on Nuxt UI since RB-080-85): the answer of the tool
// `social_preview` (plugins/seo/src/social/tool.rs) - since RB-080-99 every card with the layout
// its platform's rule picks, the other one to look at, whether the rule is unsure and why - and
// what the panel says about a card's image. Pure functions, no DOM.

/** A text of the panel below `plugin.seo.`, formatted (`usePanel().t`). */
export type Texts = (key: string, params?: Record<string, unknown>) => string;

/** A card's layout: the image above the text, beside it, or no image. */
export type Layout = 'large' | 'small' | 'text';

/** A text of a card: what it shows, the whole text, and the tag it came from. */
export interface CardText {
  text: string;
  full: string;
  truncated: boolean;
  source: string;
}

/** Why a card has its layout: `plugin.seo.panel.reasons.<code>` with `params`. */
export interface Reason {
  code: string;
  params: Record<string, string>;
}

/** One platform's preview. */
export interface Card {
  id: string;
  name: string;
  layout: Layout;
  alternate: Layout | null;
  unsure: boolean;
  reason: Reason;
  confidence: 'official' | 'code' | 'unverified';
  source: string | null;
  title: CardText | null;
  description: CardText | null;
  image: string | null;
  imageSource: string | null;
}

/** A share image as the component fetched it; `dataUri` while the answer had room. */
export interface ImageInfo {
  url: string;
  status: number | null;
  error: string | null;
  format: string | null;
  width: number | null;
  height: number | null;
  bytes: number;
  dataUri: string | null;
  tooLarge: boolean;
}

/** The tool's answer. */
export interface Preview {
  url: string;
  finalUrl: string;
  status: number | null;
  domain: string;
  dataVersion: number;
  cards: Card[];
  images: ImageInfo[];
}

const LAYOUTS: readonly string[] = ['large', 'small', 'text'];
const CONFIDENCES: readonly string[] = ['official', 'code', 'unverified'];
/** A reason code becomes part of a text key: a plain word only. */
const CODE = /^[a-z_]{1,40}$/;

/** Whether `value` is an object of string values (a reason's parameters). */
function isParams(value: unknown): value is Record<string, string> {
  return typeof value === 'object' && value !== null && !Array.isArray(value) && Object.values(value).every((one) => typeof one === 'string');
}

/** Whether `value` reads as one card (the frame trusts nothing it is sent): every field the panel
 * builds a text key or a layout from is checked. */
function isCard(value: unknown): value is Card {
  const v = value as Partial<Card> | null;
  return (
    typeof v === 'object' &&
    v !== null &&
    typeof v.id === 'string' &&
    typeof v.name === 'string' &&
    LAYOUTS.includes(String(v.layout)) &&
    (v.alternate === null || LAYOUTS.includes(String(v.alternate))) &&
    typeof v.unsure === 'boolean' &&
    CONFIDENCES.includes(String(v.confidence)) &&
    (v.source === null || typeof v.source === 'string') &&
    typeof v.reason === 'object' &&
    v.reason !== null &&
    typeof v.reason.code === 'string' &&
    CODE.test(v.reason.code) &&
    isParams(v.reason.params)
  );
}

/** Whether `value` reads as the tool's answer. */
export function isPreview(value: unknown): value is Preview {
  const v = value as Partial<Preview> | null;
  return (
    typeof v === 'object' &&
    v !== null &&
    typeof v.url === 'string' &&
    typeof v.domain === 'string' &&
    Array.isArray(v.cards) &&
    v.cards.every(isCard) &&
    Array.isArray(v.images)
  );
}

/** The variants a card shows at once: both when its rule is unsure, else the chosen one; the
 * other one (`alternate`) is behind the toggle. */
export function shownLayouts(card: Card): Layout[] {
  return card.unsure && card.alternate ? [card.layout, card.alternate] : [card.layout];
}

/** The variant behind the toggle: the other one of a sure rule. */
export function otherLayout(card: Card): Layout | null {
  return card.unsure ? null : card.alternate;
}

/** The sentence why a card has its layout. */
export function reasonText(t: Texts, card: Card): string {
  return t(`panel.reasons.${card.reason.code}`, card.reason.params);
}

/** The image a card shows. */
export function imageOf(preview: Preview, card: Card): ImageInfo | undefined {
  return card.image === null ? undefined : preview.images.find((image) => image.url === card.image);
}

/** Only an inline image may become an `<img>`: the frame's CSP allows `data:` alone. */
export function safeDataUri(image: ImageInfo | undefined): string | null {
  const uri = image?.dataUri ?? null;
  return uri !== null && /^data:image\/(png|jpeg|gif|webp);base64,[A-Za-z0-9+/=]+$/.test(uri) ? uri : null;
}

/** What a card says about its image when it cannot show it: the status it answered, or the
 * sentence of the failure (`say`, the panel's error text) - never a bare code (RB-080-85). */
export function imageNote(t: Texts, image: ImageInfo | undefined, say: (code: string) => string = (code) => code): string {
  if (!image) return t('panel.no_image');
  if (image.status !== 200) {
    return t('panel.image_failed', { reason: image.status !== null ? image.status : say(image.error ?? '') });
  }
  if (image.tooLarge) return t('panel.image_large', { bytes: image.bytes });
  return '';
}

/** The image's size line (`1200 x 630 image/jpeg`); empty without a size. */
export function imageSize(t: Texts, image: ImageInfo | undefined): string {
  if (!image || image.width === null || image.height === null) return '';
  return t('panel.image_size', { width: image.width, height: image.height, format: image.format ?? '' });
}

/** A rule's source when the panel may open it: only `https:` (the host's `open_link` takes
 * `http:` and `https:`; the table holds `https:` alone). */
export function sourceLink(url: string | null): string | null {
  if (!url) return null;
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'https:' ? parsed.href : null;
  } catch {
    return null;
  }
}
