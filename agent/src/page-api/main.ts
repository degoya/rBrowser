// The main-world bundle's entry (dist/rb-page-api.js, RB-020-08): the core wraps it in
// `(function(__rbPageBoot){...})(<PageState>)`, so the state never becomes a global of the page.

import { installPageApi } from './main-world';

declare const __rbPageBoot: unknown;

installPageApi(window, typeof __rbPageBoot === 'undefined' ? undefined : __rbPageBoot);
