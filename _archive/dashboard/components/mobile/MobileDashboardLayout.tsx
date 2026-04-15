"use client";

import { Outlet, useLocation } from "next/navigation";
import MobileNavbar from "./Navbar";
import MobileSidebar from "./Sidebar";
import { useState } from "react";

export default function MobileDashboardLayout() {
  const location = useLocation();
  const [sidebarOpen, setSidebarOpen] = useState(false);

  return (
    <div className="min-h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white border-b border-gray-200 sticky top-0 z-40">
        <div className="flex items-center justify-between px-4 py-3">
          <button
            onClick={() => setSidebarOpen(true)}
            className="lg:hidden p-2 -ml-2 rounded-md"
          >
            <svg
              className="h-6 w-6 text-gray-600"
              fill="none"
              viewBox="0 0 24 24"
              strokeWidth={2}
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M4 6h16M4 12h16M4 18h16"
              />
            </svg>
          </button>
          <div className="flex-1 flex justify-center">
            <h1 className="text-lg font-semibold text-gray-900">
              FlowLink
            </h1>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-blue-600 rounded-full flex items-center justify-center text-white font-medium">
              {user.name.charAt(0)}
            </div>
          </div>
        </div>
      </header>

      {/* Main content */}
      <main className="pb-20">
        <Outlet />
      </main>

      {/* Mobile sidebar */}
      <MobileSidebar
        isOpen={sidebarOpen}
        onClose={() => setSidebarOpen(false)}
        user={user}
      />

      {/* Mobile bottom navbar */}
      <MobileNavbar />
    </div>
  );
}

// Mock user data
const user = {
  name: "Александр",
  email: "alexander@example.com",
  avatar: undefined,
};