"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/Button";

interface School {
  id: string;
  name: string;
  code: string;
  status: string;
  created_at: string;
}

interface SuperAdminStats {
  schools: School[];
}

export default function SuperAdminPage() {
  const router = useRouter();
  const [stats, setStats] = useState<SuperAdminStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [impersonating, setImpersonating] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/v1/auth/super-admin/stats")
      .then(res => res.json())
      .then(data => {
        setStats(data);
        setLoading(false);
      })
      .catch(err => {
        console.error(err);
        setLoading(false);
      });
  }, []);

  const handleImpersonate = async (schoolId: string) => {
    setImpersonating(schoolId);
    try {
      const res = await fetch("/api/v1/auth/switch-tenant", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ school_id: schoolId })
      });
      if (res.ok) {
        window.location.href = "/dashboard";
      } else {
        alert("Failed to switch tenant.");
        setImpersonating(null);
      }
    } catch (err) {
      console.error(err);
      setImpersonating(null);
    }
  };

  return (
    <div className="max-w-6xl mx-auto py-8 px-4">
      <div className="flex justify-between items-center mb-8">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 mb-1">Super Admin Overview</h1>
          <p className="text-sm text-gray-500">Manage school tenants and system access.</p>
        </div>
        <Button onClick={() => router.push("/dashboard/super-admin/schools/new")}>
          + Add New School
        </Button>
      </div>

      <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <div className="px-6 py-4 border-b border-gray-200 flex justify-between items-center bg-gray-50/50">
          <h2 className="text-sm font-bold text-gray-700 uppercase tracking-wider">Active School Tenants</h2>
        </div>
        <div className="p-0">
          {loading ? (
            <div className="p-6 text-sm text-gray-500 animate-pulse">Loading tenants...</div>
          ) : (
            <table className="w-full text-sm text-left">
              <thead className="text-xs text-gray-500 bg-gray-50 border-b border-gray-200">
                <tr>
                  <th className="px-6 py-3 font-medium">School Name</th>
                  <th className="px-6 py-3 font-medium">Code</th>
                  <th className="px-6 py-3 font-medium">Status</th>
                  <th className="px-6 py-3 font-medium text-right">Created</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {stats?.schools?.map(s => (
                  <tr 
                    key={s.id} 
                    onClick={() => handleImpersonate(s.id)}
                    className="group cursor-pointer hover:bg-gray-50 transition-colors"
                  >
                    <td className="px-6 py-4 font-medium text-gray-900 flex items-center gap-3">
                      <div className="w-8 h-8 rounded-md bg-[#0C152E] flex items-center justify-center text-white font-bold text-xs">
                        {s.name.substring(0,2).toUpperCase()}
                      </div>
                      <div>
                        {s.name}
                        {impersonating === s.id && <span className="ml-2 text-xs text-blue-600 font-semibold animate-pulse">Switching...</span>}
                      </div>
                    </td>
                    <td className="px-6 py-4 text-gray-500 font-mono text-xs">{s.code}</td>
                    <td className="px-6 py-4">
                      <span className={`text-[10px] font-bold px-2.5 py-1 rounded-full uppercase tracking-wider ${
                        s.status === 'ACTIVE' ? 'bg-green-100 text-green-700' : 
                        s.status === 'PENDING_SETUP' ? 'bg-orange-100 text-orange-700' :
                        'bg-red-100 text-red-700'
                      }`}>
                        {s.status}
                      </span>
                    </td>
                    <td className="px-6 py-4 text-right text-gray-500">
                      {new Date(s.created_at).toLocaleDateString()}
                    </td>
                  </tr>
                ))}
                {stats?.schools?.length === 0 && (
                  <tr>
                    <td colSpan={4} className="px-6 py-8 text-center text-gray-500">No schools found. Create one to get started.</td>
                  </tr>
                )}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}
