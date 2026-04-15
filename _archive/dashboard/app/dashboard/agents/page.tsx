"use client";

import { useState } from "react";
import Link from "next/link";

interface Agent {
  id: string;
  name: string;
  status: "online" | "offline" | "busy";
  lastActive: string;
  region: string;
}

export default function MobileAgentsPage() {
  const [selectedAgent, setSelectedAgent] = useState<Agent | null>(null);

  const agents: Agent[] = [
    {
      id: "server-1",
      name: "Production Server 1",
      status: "online",
      lastActive: "2 min ago",
      region: "EU-West",
    },
    {
      id: "server-2",
      name: "Staging Server",
      status: "offline",
      lastActive: "1 hour ago",
      region: "EU-West",
    },
    {
      id: "server-3",
      name: "Backup Server",
      status: "busy",
      lastActive: "Just now",
      region: "US-East",
    },
  ];

  return (
    <div className="pb-24 px-4 py-6">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900 mb-2">
          Agents
        </h1>
        <p className="text-gray-600">
          Manage your connected agents
        </p>
      </div>

      {/* Add Agent Button */}
      <button className="w-full bg-blue-600 text-white py-3 px-4 rounded-lg font-medium mb-6 flex items-center justify-center gap-2">
        <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
        </svg>
        Add New Agent
      </button>

      {/* Agent List */}
      <div className="space-y-3">
        {agents.map((agent) => (
          <div
            key={agent.id}
            onClick={() => setSelectedAgent(agent)}
            className={`bg-white rounded-lg p-4 border-2 ${
              selectedAgent?.id === agent.id
                ? "border-blue-600"
                : "border-gray-200"
            } cursor-pointer`}
          >
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-3">
                <div className={`w-3 h-3 rounded-full ${
                  agent.status === "online" ? "bg-green-500" :
                  agent.status === "offline" ? "bg-red-500" :
                  "bg-yellow-500"
                }`} />
                <h3 className="font-semibold text-gray-900">
                  {agent.name}
                </h3>
              </div>
              <span className="text-sm text-gray-500">
                {agent.lastActive}
              </span>
            </div>
            <div className="flex items-center justify-between text-sm text-gray-600">
              <span className="flex items-center gap-1">
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 11a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
                {agent.region}
              </span>
              <span className={`text-xs px-2 py-1 rounded-full ${
                agent.status === "online" ? "bg-green-100 text-green-700" :
                agent.status === "offline" ? "bg-red-100 text-red-700" :
                "bg-yellow-100 text-yellow-700"
              }`}>
                {agent.status}
              </span>
            </div>
          </div>
        ))}
      </div>

      {/* Agent Details Modal */}
      {selectedAgent && (
        <AgentDetailsModal
          agent={selectedAgent}
          onClose={() => setSelectedAgent(null)}
        />
      )}
    </div>
  );
}

function AgentDetailsModal({ agent, onClose }: { agent: Agent; onClose: () => void }) {
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-end sm:items-center justify-center">
      <div className="bg-white rounded-t-3xl sm:rounded-2xl w-full sm:max-w-lg p-6 max-h-[80vh] overflow-y-auto">
        <div className="flex items-start justify-between mb-6">
          <div>
            <h2 className="text-xl font-bold text-gray-900 mb-1">
              {agent.name}
            </h2>
            <p className="text-sm text-gray-600">ID: {agent.id}</p>
          </div>
          <button
            onClick={onClose}
            className="p-2 text-gray-400 hover:text-gray-600"
          >
            <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="space-y-4">
          {/* Status */}
          <div className="bg-gray-50 rounded-lg p-4">
            <h3 className="text-sm font-medium text-gray-900 mb-2">
              Status
            </h3>
            <div className="flex items-center gap-2">
              <div className={`w-3 h-3 rounded-full ${
                agent.status === "online" ? "bg-green-500" :
                agent.status === "offline" ? "bg-red-500" :
                "bg-yellow-500"
              }`} />
              <span className="text-sm text-gray-600">
                {agent.status}
              </span>
            </div>
          </div>

          {/* Last Active */}
          <div className="bg-gray-50 rounded-lg p-4">
            <h3 className="text-sm font-medium text-gray-900 mb-2">
              Last Active
            </h3>
            <p className="text-sm text-gray-600">
              {agent.lastActive}
            </p>
          </div>

          {/* Region */}
          <div className="bg-gray-50 rounded-lg p-4">
            <h3 className="text-sm font-medium text-gray-900 mb-2">
              Region
            </h3>
            <p className="text-sm text-gray-600">
              {agent.region}
            </p>
          </div>

          {/* Actions */}
          <div className="flex gap-2">
            <button className="flex-1 bg-blue-600 text-white py-3 px-4 rounded-lg font-medium">
              Connect
            </button>
            <button className="flex-1 bg-white border border-gray-300 text-gray-700 py-3 px-4 rounded-lg font-medium">
              Disconnect
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}