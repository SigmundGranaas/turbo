import { NavLink, Outlet } from "react-router-dom";
import { RESOURCES } from "../api/types";

const RESOURCE_LABEL: Record<string, string> = {
  "hiking-trails": "Hiking trails",
  "ski-tracks": "Ski tracks",
  "forest-roads": "Forest roads",
  "cycling-routes": "Cycling routes",
};

const linkClass = ({ isActive }: { isActive: boolean }) =>
  `block px-3 py-2 rounded text-sm ${
    isActive
      ? "bg-ink-900 text-ink-50 font-medium"
      : "text-ink-700 hover:bg-ink-200"
  }`;

export function AppShell() {
  return (
    <div className="min-h-screen flex">
      <aside className="w-56 bg-ink-100 border-r border-ink-200 flex flex-col">
        <div className="px-4 py-4 border-b border-ink-200">
          <div className="font-semibold tracking-tight">Turbo Admin</div>
          <div className="text-xs text-ink-500">Curated paths</div>
        </div>
        <nav className="flex-1 p-2 space-y-1">
          <NavLink to="/" end className={linkClass}>
            Dashboard
          </NavLink>
          <div className="pt-3 pb-1 px-3 text-xs uppercase tracking-wide text-ink-500">
            Resources
          </div>
          {RESOURCES.map((r) => (
            <NavLink key={r} to={`/resources/${r}`} className={linkClass}>
              {RESOURCE_LABEL[r]}
            </NavLink>
          ))}
          <div className="pt-3 pb-1 px-3 text-xs uppercase tracking-wide text-ink-500">
            Tools
          </div>
          <NavLink to="/upload-gpx" className={linkClass}>
            Upload GPX
          </NavLink>
          <NavLink to="/upload-bulk" className={linkClass}>
            Upload bulk dataset
          </NavLink>
          <NavLink to="/incoming" className={linkClass}>
            Incoming files
          </NavLink>
          <NavLink to="/jobs" className={linkClass}>
            Ingest jobs
          </NavLink>
        </nav>
        <div className="p-3 text-xs text-ink-500 border-t border-ink-200">
          Signed in via Turbo auth
        </div>
      </aside>
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
