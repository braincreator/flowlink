"use client";

import Link from "next/link";
import { useState } from "react";

interface NavbarProps {
  user: {
    name: string;
    email: string;
    avatar?: string;
  };
}

export default function MobileNavbar({ user }: NavbarProps) {
  const [isOpen, setIsOpen] = useState(false);

  const navigation = [
    { name: "Dashboard", href: "/dashboard", icon: "📊" },
    { name: "Agents", href: "/dashboard/agents", icon: "🤖" },
    { name: "Webhooks", href: "/dashboard/webhooks", icon: "🔗" },
    { name: "Billing", href: "/dashboard/billing", icon: "💳" },
  ];

  return (
    <nav className="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200 z-50 sm:hidden">
      <div className="flex justify-around items-center py-3">
        {navigation.map((item) => (
          <Link
            key={item.name}
            href={item.href}
            className="flex flex-col items-center text-gray-600 hover:text-blue-600 transition-colors"
          >
            <span className="text-xl">{item.icon}</span>
            <span className="text-xs mt-1">{item.name}</span>
          </Link>
        ))}
      </div>
    </nav>
  );
}