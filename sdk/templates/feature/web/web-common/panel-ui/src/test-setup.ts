// The tests' frame has no network either: an icon is never fetched from the Iconify API.
import { _api as iconifyApi } from '@iconify/vue';

iconifyApi.setFetch(() => Promise.reject(new Error('icons are bundled')));
