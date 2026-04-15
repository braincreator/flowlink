"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { LucideIcon } from "lucide-react";

interface MobileStatCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  icon?: LucideIcon;
  trend?: {
    value: number;
    positive: boolean;
  };
  color?: "blue" | "green" | "yellow" | "red";
}

export default function MobileStatCard({
  title,
  value,
  subtitle,
  icon: Icon,
  trend,
  color = "blue",
}: MobileStatCardProps) {
  const colorClasses = {
    blue: "bg-blue-50 text-blue-600",
    green: "bg-green-50 text-green-600",
    yellow: "bg-yellow-50 text-yellow-600",
    red: "bg-red-50 text-red-600",
  };

  const bgClasses = {
    blue: "border-blue-200",
    green: "border-green-200",
    yellow: "border-yellow-200",
    red: "border-red-200",
  };

  const trendClasses = {
    positive: "text-green-600",
    negative: "text-red-600",
  };

  return (
    <Card className={bgClasses[color]}>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm font-medium text-gray-600">
            {title}
          </CardTitle>
          {Icon && (
            <div className={`p-2 rounded-lg ${colorClasses[color]}`}>
              <Icon className="w-4 h-4" />
            </div>
          )}
        </div>
      </CardHeader>
      <CardContent>
        <div className="text-3xl font-bold text-gray-900 mb-1">
          {value}
        </div>
        {subtitle && (
          <p className="text-sm text-gray-600 mb-2">{subtitle}</p>
        )}
        {trend && (
          <div className="flex items-center gap-1 text-sm">
            <span className={trendClasses[trend.positive ? "positive" : "negative"]}>
              {trend.positive ? "↑" : "↓"}
            </span>
            <span className={trendClasses[trend.positive ? "positive" : "negative"]}>
              {Math.abs(trend.value)}%
            </span>
            <span className="text-gray-500 text-xs">from last month</span>
          </div>
        )}
      </CardContent>
    </Card>
  );
}