/** Backend base URL. Defaults to production; CORS there whitelists the Vite
 *  dev origin (localhost:5173) with credentials, so the dev server talks to it
 *  directly with no proxy. Override with `VITE_API_BASE`. */
export const API_BASE = (import.meta.env.VITE_API_BASE ?? 'https://kart-api.sandring.no').replace(
  /\/$/,
  '',
);
