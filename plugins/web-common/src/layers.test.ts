// web-common's copy of the overlays' one host (RB-090-51): the bundled page scripts and the
// code a component runs through `evaluate` use it.
import { addLayer, removeLayer } from './layers';
import { describeLayers } from './layers-spec';

describeLayers('layers.ts', (owner) => ({ add: (host) => addLayer(host, owner), remove: removeLayer }));
