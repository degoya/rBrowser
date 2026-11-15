// The bundle's entry (dist/rb-agent.js, one IIFE): boots the agent in the world it runs in.

import { boot } from './boot';

boot(globalThis as unknown as Record<string, unknown>, window);
