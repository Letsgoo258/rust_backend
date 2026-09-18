"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/Button";

interface School {
  id: string;
  name: string;
  code: string;
  status: string;
  created_at: string;
}

interface StatsResponse {
  total_users: number;
  active_tenants: number;
  schools: School[];
}

export default function SuperAdminDashboard() {
  const router = useRouter();

  const handleImpersonate = async (schoolId: string) => {
    try {
      const res = await fetch("/api/v1/auth/switch-tenant", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ school_id: schoolId })
      });
      if (res.ok) {
        window.location.href = "/dashboard";
      }
    } catch (err) {
      console.error(err);
    }
  };

  const [stats, setStats] = useState<StatsResponse | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch("/api/v1/auth/schools")
      .then((res) => {
        if (!res.ok) throw new Error("Failed to fetch stats");
        return res.json();
      })
      .then((data) => {
        setStats(data);
      })
      .catch((err) => {
        console.error(err);
      })
      .finally(() => {
        setLoading(false);
      });
  }, []);

  return (
    <div className="space-y-8 animate-in fade-in duration-500">
      
      {/* Overview Header Area */}
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold text-gray-900 flex items-center gap-2">
          Platform Overview <span className="text-xs font-medium text-gray-400 font-mono tracking-wider">Just now <svg className="w-3 h-3 inline pb-px" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path></svg></span>
        </h1>
        <Link href="/dashboard/super-admin/schools/new" className="text-sm font-medium text-[#3366FF] hover:underline flex items-center gap-1">
          Add new school tenant <span aria-hidden="true">&rarr;</span>
        </Link>
      </div>

      {/* Main Metric Card */}
      <div className="bg-white rounded-xl shadow-sm border border-gray-200/60 overflow-hidden flex flex-col lg:flex-row">
        
        {/* Left Side: Stats */}
        <div className="flex-1 p-6 lg:p-8 flex flex-col sm:flex-row gap-8 lg:gap-16 border-b lg:border-b-0 lg:border-r border-gray-100">
          <div>
            <div className="w-8 h-8 rounded bg-blue-50 flex items-center justify-center mb-3">
              <svg className="w-4 h-4 text-[#3366FF]" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"></path></svg>
            </div>
            <p className="text-sm font-medium text-gray-800 mb-1 flex items-center gap-1">
              Active Tenants <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 5l7 7-7 7"></path></svg>
            </p>
            <p className="text-xs text-gray-500 mb-4">Updated just now</p>
            <div className="flex items-baseline gap-1">
              <span className="text-2xl font-bold text-gray-900">{loading ? "-" : stats?.active_tenants || 0}</span>
            </div>
            <p className="text-xs font-semibold text-gray-900 underline underline-offset-4 decoration-gray-300 decoration-2 mt-4 cursor-pointer hover:decoration-gray-400">View tenant breakup</p>
          </div>

          <div>
            <div className="w-8 h-8 rounded bg-green-50 flex items-center justify-center mb-3">
              <svg className="w-4 h-4 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
            </div>
            <p className="text-sm font-medium text-gray-800 mb-1">System Health</p>
            <p className="text-xs text-gray-500 mb-4">All services operational</p>
          </div>

          <div>
            <div className="w-8 h-8 rounded bg-gray-50 flex items-center justify-center mb-3 border border-gray-100">
              <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"></path></svg>
            </div>
            <p className="text-sm font-medium text-gray-800 mb-1">Next Maintenance</p>
            <p className="text-xs text-gray-500 mb-4">No upcoming maintenance</p>
            <p className="text-xs text-orange-500 font-medium flex items-center gap-1 mt-4">
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path></svg>
              No downtime scheduled
            </p>
          </div>
        </div>

        {/* Right Side: Action/Balance */}
        <div className="p-6 lg:p-8 bg-[#FAFAFA] min-w-[280px]">
          <p className="text-xs font-bold text-gray-600 border-b border-gray-300 pb-1 mb-4 inline-block border-dashed">Total Users</p>
          <div className="flex items-baseline mb-6">
            <span className="text-3xl font-bold text-gray-900 tracking-tight">{loading ? "-" : stats?.total_users || 0}</span>
          </div>
          
          <p className="text-xs font-semibold text-gray-800 mb-2">Need a system report?</p>
          <button className="w-full bg-white border border-gray-200 text-gray-400 font-medium py-2 rounded-lg text-sm shadow-sm flex items-center justify-center gap-2 cursor-not-allowed">
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"></path></svg>
            Generate Report
          </button>
        </div>
      </div>

      {/* Tenants Table */}
      <div>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-gray-900">School Tenants Overview</h2>
          <div className="flex gap-2">
            <button className="bg-white border border-gray-200 text-gray-700 text-sm font-medium px-3 py-1.5 rounded-md flex items-center gap-2 shadow-sm">
              <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"></path></svg>
              This Month <svg className="w-3 h-3 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7"></path></svg>
            </button>
          </div>
        </div>
        
        <div className="bg-white rounded-xl shadow-sm border border-gray-200/60 overflow-hidden">
          <div className="grid grid-cols-3 border-b border-gray-200">
            <div className="p-4 border-r border-gray-200">
              <p className="text-xs font-medium text-gray-500 mb-1 flex items-center gap-1">Total Tenants <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg></p>
              <p className="text-lg font-bold text-gray-900">{loading ? "-" : stats?.schools?.length || 0}</p>
            </div>
            <div className="p-4 border-r border-gray-200">
              <p className="text-xs font-medium text-gray-500 mb-1 flex items-center gap-1">Inactive <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg></p>
              <p className="text-lg font-bold text-gray-900">{loading ? "-" : stats?.schools?.filter(s => s.status !== 'ACTIVE').length || 0}</p>
            </div>
            <div className="p-4">
              <p className="text-xs font-medium text-gray-500 mb-1 flex items-center gap-1">Total Active Users <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg></p>
              <p className="text-lg font-bold text-gray-900">{loading ? "-" : stats?.total_users || 0}</p>
            </div>
          </div>
          
          <div className="p-6">
            {loading ? (
              <div className="animate-pulse space-y-4">
                <div className="h-4 bg-gray-200 rounded w-1/4"></div>
                <div className="h-4 bg-gray-200 rounded w-1/2"></div>
              </div>
            ) : (
              <table className="w-full text-sm text-left">
                <thead className="text-xs text-gray-500 bg-gray-50 border-b border-gray-200">
                  <tr>
                    <th className="px-4 py-3 font-medium">School Name</th>
                    <th className="px-4 py-3 font-medium">Code</th>
                    <th className="px-4 py-3 font-medium">Status</th>
                    <th className="px-4 py-3 font-medium text-right">Created</th>
                  </tr>
                </thead>
                <tbody>
                  {stats?.schools?.map(s => (
                    <tr key={s.id} className="border-b border-gray-100 last:border-0 hover:bg-gray-50 transition-colors">
                      <td className="px-4 py-4 font-medium text-gray-900">{s.name}</td>
                      <td className="px-4 py-4 text-gray-500 font-mono text-xs">{s.code}</td>
                      <td className="px-4 py-4">
                        <span className="bg-green-100 text-green-700 text-[10px] font-bold px-2 py-1 rounded-full uppercase tracking-wider">{s.status}</span>
                      </td>
                      <td className="px-4 py-4 text-right text-gray-500">{new Date(s.created_at).toLocaleDateString()}</td>
                    </tr>
                  ))}
                  {stats?.schools?.length === 0 && (
                    <tr>
                      <td colSpan={4} className="px-4 py-8 text-center text-gray-500">No school tenants found.</td>
                    </tr>
                  )}
                </tbody>
              </table>
            )}
          </div>
        </div>
      </div>
      
    </div>
  );
}
