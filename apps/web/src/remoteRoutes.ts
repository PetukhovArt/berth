type Route = { kind: string; url: string; entry: string };
type Request = (url: string, init: RequestInit) => Promise<Response>;

const ROUTES_PATH = '/__berth/routes';
const HEALTH_PATH = '__berth/health';
const STORAGE_KEY = 'berth.remote.routes';
const PROBE_TIMEOUT_MS = 3000;
const RECHECK_MS = 15000;
const RETRY_MS = 2000;

function stored(): Route[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
    return Array.isArray(parsed) ? (parsed as Route[]) : [];
  } catch {
    return [];
  }
}

async function fetchRoutes(request: Request): Promise<Route[]> {
  try {
    const response = await request(ROUTES_PATH, { credentials: 'same-origin', cache: 'no-store' });
    if (!response.ok) return stored();
    const routes = (await response.json()) as Route[];
    localStorage.setItem(STORAGE_KEY, JSON.stringify(routes));
    return routes;
  } catch {
    return stored();
  }
}

async function answers(request: Request, route: Route): Promise<boolean> {
  try {
    await request(`${route.url}${HEALTH_PATH}`, { mode: 'no-cors', cache: 'no-store', signal: AbortSignal.timeout(PROBE_TIMEOUT_MS) });
    return true;
  } catch {
    return false;
  }
}

function isCurrent(route: Route): boolean {
  return new URL(route.url).origin === location.origin;
}

async function firstAnswering(request: Request, routes: Route[], stopAtCurrent: boolean): Promise<Route | null> {
  for (const route of routes) {
    if (stopAtCurrent && isCurrent(route)) return null;
    if (await answers(request, route)) return route;
  }
  return null;
}

function go(route: Route) {
  if (isCurrent(route)) location.reload();
  else location.assign(route.entry);
}

export function watchRoutes(request: Request) {
  const check = async () => {
    const better = await firstAnswering(request, await fetchRoutes(request), true);
    if (better) go(better);
  };
  void check();
  setInterval(() => void check(), RECHECK_MS);
}

export function recoverRoute(request: Request) {
  const attempt = async () => {
    const route = await firstAnswering(request, stored(), false);
    if (route) go(route);
    else setTimeout(() => void attempt(), RETRY_MS);
  };
  setTimeout(() => void attempt(), RETRY_MS);
}
