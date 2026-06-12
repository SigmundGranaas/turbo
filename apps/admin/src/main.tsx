import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createBrowserRouter } from "react-router-dom";

import { AppShell } from "./app/AppShell";
import { AuthGate } from "./app/AuthGate";
import { Dashboard } from "./screens/Dashboard";
import { ResourceList } from "./screens/ResourceList";
import { ResourceEdit } from "./screens/ResourceEdit";
import { ResourceCreate } from "./screens/ResourceCreate";
import { UploadGpx } from "./screens/UploadGpx";
import { UploadBulk } from "./screens/UploadBulk";
import { Incoming } from "./screens/Incoming";
import { Provision } from "./screens/Provision";
import { Jobs } from "./screens/Jobs";
import { PlotRoute } from "./screens/PlotRoute";
import { Primitives } from "./screens/Primitives";

import "./index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Long staleTime so toggling between screens reuses cached data;
      // mutations invalidate the relevant queries explicitly.
      staleTime: 30_000,
      refetchOnWindowFocus: false,
    },
  },
});

// `basename` matches Vite's `base` so deep links land on the right
// component when the SPA is served at /admin/app behind the gateway.
const router = createBrowserRouter(
  [
    {
      path: "/",
      element: (
        <AuthGate>
          <AppShell />
        </AuthGate>
      ),
      children: [
        { index: true, element: <PlotRoute /> },
        { path: "dashboard", element: <Dashboard /> },
        { path: "plot", element: <PlotRoute /> },
        { path: "resources/:resource", element: <ResourceList /> },
        { path: "resources/:resource/new", element: <ResourceCreate /> },
        { path: "resources/:resource/:id", element: <ResourceEdit /> },
        { path: "upload-gpx", element: <UploadGpx /> },
        { path: "upload-bulk", element: <UploadBulk /> },
        { path: "incoming", element: <Incoming /> },
        { path: "provision", element: <Provision /> },
        { path: "jobs", element: <Jobs /> },
        { path: "primitives", element: <Primitives /> },
      ],
    },
  ],
  { basename: "/admin/app" },
);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </React.StrictMode>,
);
